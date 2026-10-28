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
- sequential multi-source scanning through one Core batch invocation;
- redaction and templating;
- keyed pseudonymization and deterministic synthesis;
- Core-backed in-memory Share Bundles in the Flow configuration.

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
| Core | `--no-default-features` | Scanner, built-ins, custom rules, findings, candidates, explanations, reporting, admitted byte streams, incremental documents and sequential batches |
| Flow | `transform` (enabled by default) | Core plus all existing transformation APIs and Share Bundles |

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
Streaming, incremental sessions and sequential batches are available in both
configurations. Share Bundles are Flow-only.

## Custom rule remediation

Both variants preserve `addLiteral`, `addPrefix`, `addSuffix` and `addPattern`
with their existing three-argument signatures and no-remediation default. The
following additive builder methods accept optional Core remediation:

```ts
addLiteralWithRemediation(id: string, value: string, severity: FindingSeverity,
  remediation?: RemediationKind | null): void;
addPrefixWithRemediation(id: string, value: string, severity: FindingSeverity,
  remediation?: RemediationKind | null): void;
addSuffixWithRemediation(id: string, value: string, severity: FindingSeverity,
  remediation?: RemediationKind | null): void;
addPatternWithRemediation(id: string, pattern: string, severity: FindingSeverity,
  remediation?: RemediationKind | null): void;
```

Omitted, `undefined`, `null` and `RemediationKind.None` all retain no remediation.
The seven concrete `RemediationKind` values map directly to Core `Remediation`
and `Rule.with_remediation`; no validation or matching policy is attached.
`Unknown` is output-only and throws a JavaScript error without staging a rule.
Invalid patterns are rejected first by Core's constructor, and empty/duplicate
IDs and empty literal/prefix/suffix matchers retain Core's build-time validation.
Configuration diagnostics may mention configuration inputs; no scanned source
is passed to construction diagnostics. All existing enum values are preserved.

```js
const builder = new ScanEngineBuilder(false);
builder.addLiteralWithRemediation("app.secret", "SECRET", FindingSeverity.High,
  RemediationKind.RemoveSensitiveValue);
const engine = builder.build();
// Findings expose Core's remediation through the existing result projection.
engine.free();
```

Custom remediation is retained by scan, stream and incremental results and by
existing explanations and Flow transforms. It changes metadata only. Catalog
composition remains available through `new ScanEngineBuilder(include_builtins)`,
`addFinancialBuiltins()` and `addPersonalBuiltins()`, delegating to Core.

## Collection indices

`ScanResult.findingAt(index)`, `candidateAt(index)`,
`candidateExplanationAt(index)`, `ScanEngine.explainFinding(result, index)`,
`ScanBatchResult.entryAt(index)` and Flow's `ShareBundle.entryAt(index)` validate
the original JavaScript value before conversion to a Rust index. Their generated
TypeScript arguments remain `index: number` and their return types are unchanged.

An index must be a finite, nonnegative safe integer representable as WASM32
`usize` (`0` through `4294967295`). Negative and fractional values, `NaN`, either
infinity, unsafe integers, target-width overflow and non-number values throw a
source-free native JavaScript `Error`, without coercion, truncation or wrapping.
JavaScript negative zero is accepted as zero. Representable integers outside the
collection still throw the existing collection-specific out-of-range error.
Errors need no `free()` and failed lookups do not consume or mutate their owner.
Successful lookups retain their existing independent ownership and `free()` rules.

The Core/Flow generated-runtime qualification tests every accessor against 16
invalid numeric values and 10 non-number values, valid zero/negative-zero/one
indices, collection bounds and the maximum WASM32 index. It also checks privacy,
continued usability after errors and returned views after parent/sibling frees.

This validation is specific to collection indices. Streaming/incremental limits
and edit offsets retain their existing strict validation. Flow pseudonymization
digest-length parameters retain their existing wasm-bindgen numeric conversion
and Core clamping; they are not collection indices.

## Active rule metadata and structured build errors

`ScanEngine.activeRules(): RuleMetadataView[]` copies Core's active metadata in
Core order, without sorting or retaining another rule inventory. Each view has
five readonly properties: `ruleId: string`, `kind: RuleKind`,
`detectionMode: DetectionModeKind`, `severity: FindingSeverity` and
`remediation: RemediationKind` (`None` when absent). The new `RuleKind` enum has
`Literal = 0`, `Prefix = 1`, `Suffix = 2` and `Pattern = 3`; the other enums are
unchanged. No matcher payloads, patterns, validators or scanned source are exposed.

Each call returns a new JavaScript array of independently owned wasm-bindgen
views. They outlive scans, reports and the engine. Free every view separately;
the array and strings returned by getters are ordinary garbage-collected JS values:

```js
const rules = engine.activeRules();
try {
  for (const rule of rules) console.log(rule.ruleId, rule.kind, rule.remediation);
} finally {
  for (const rule of rules) rule.free();
}
```

All existing builder signatures remain unchanged, including `addPattern()` and
consuming `build()`. Their failures remain native JavaScript `Error` objects with
the same Core diagnostic messages. Construction failures are now enriched with
`code: ScannerBuildErrorCode` and `ruleId: string | undefined`. The type-only
`ScannerBuildError extends Error` interface describes these properties; it has
no JS constructor, and caught build errors need no `free()`. Streaming and
incremental error ownership rules remain as documented in their sections.

```ts
try {
  builder.build(); // consumes builder on failure too
} catch (error) {
  if (!(error instanceof Error) || !("code" in error)) throw error;
  const failure = error as ScannerBuildError;
  if (failure.code === ScannerBuildErrorCode.DuplicateRuleId) {
    console.error("duplicate rule ID", failure.ruleId);
  }
}
```

Stable categories derived from Core are `InvalidPattern`, `PatternMatchesEmpty`,
`MissingCaptureGroup`, `EmptyRuleId`, `DuplicateRuleId`, `EmptyMatcher`,
`TooManyRules` and `AutomatonBuild` (values 0 through 7 respectively).
`InvalidRemediation = 8` represents the existing rejection of the output-only
`RemediationKind.Unknown`. Categories are selected from typed Rust variants,
never parsed from messages. IDs come from Core or the known pattern-constructor
context; failures without an identity use `undefined`.

Pattern failures do not stage a rule or consume the builder. Empty matchers and
IDs, duplicate IDs and catalog collisions retain Core's build-time validation
and precedence. Diagnostics contain no scanned source. Core regex diagnostics
may contain the caller's configuration pattern; they remain human-readable text,
not machine codes. Metadata/errors are available in Core and Flow with no new
runtime dependency or WASM parallelism.

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

## Sequential batch scanning

`ScanEngine.scanBatch(sources: ScanBatchSource[])` scans several explicitly
identified UTF-8 sources in one synchronous call. The adapter copies the input
IDs and source strings once, then delegates the complete ordered batch to exactly
one Core `Scanner::scan()` invocation. It does not loop over `scan()`, duplicate
Core aggregation, or run sources in parallel.

The generated TypeScript API is:

```ts
export interface ScanBatchSource {
    readonly id: string;
    readonly source: string;
}

scanBatch(sources: ScanBatchSource[]): ScanBatchResult;

export class ScanBatchResult {
    readonly length: number;
    isEmpty(): boolean;
    entryAt(index: number): ScanBatchEntry;
    summary(): ScanSummaryView;
    free(): void;
}

export class ScanBatchEntry {
    readonly id: string;
    readonly sourceBytes: number;
    readonly result: ScanResult;
    free(): void;
}

export class ScanSummaryView {
    readonly scannedSources: number;
    readonly scannedBytes: number;
    readonly reportsWithFindings: number;
    readonly reportsWithoutFindings: number;
    readonly totalFindings: number;
    readonly totalCandidates: number;
    readonly reportsWithCandidates: number;
    readonly hasCandidates: boolean;
    readonly critical: number;
    readonly high: number;
    readonly medium: number;
    readonly low: number;
    readonly info: number;
    readonly isClean: boolean;
    readonly hasCritical: boolean;
    bySeverity(severity: FindingSeverity): number;
    free(): void;
}
```

`ScanBatchSource` is a type-only interface with no JavaScript constructor.
`ScanBatchResult`, `ScanBatchEntry` and `ScanSummaryView` are owned
wasm-bindgen objects, each with generated `free()` and `[Symbol.dispose]()`
methods. Empty, Unicode and duplicate IDs are preserved exactly; IDs are opaque
to Core. An empty array is a valid empty batch. Missing or non-string `id` or
`source` properties, and non-object elements, throw a native JavaScript `Error`
before any batch is created. Inputs are UTF-8 strings as projected by
`wasm-bindgen`; `sourceBytes`, finding offsets and summary byte totals always use
UTF-8 byte lengths.

```js
const engine = new ScanEngine();
const batch = engine.scanBatch([
  { id: "checkout", source: "card_number=1234567890123452" },
  { id: "review", source: "ABCD-EFGH-IJKL-MNOP" },
  { id: "", source: "" },
]);
try {
  console.log(batch.length, batch.isEmpty());
  for (let index = 0; index < batch.length; index += 1) {
    const entry = batch.entryAt(index);
    const result = entry.result; // fresh owned ScanResult per access
    try {
      console.log(entry.id, entry.sourceBytes, result.findingCount(),
        result.candidateCount());
      // findingAt, candidateAt, explanations and Flow transforms are unchanged.
    } finally {
      result.free();
      entry.free();
    }
  }
  const summary = batch.summary(); // fresh owned view per call
  try {
    console.log(summary.scannedSources, summary.scannedBytes,
      summary.totalFindings, summary.totalCandidates, summary.isClean);
  } finally {
    summary.free();
  }
} finally {
  batch.free();
}
engine.free();
```

Ordering and ownership are explicit:

- entries and summaries preserve input order; `entryAt(index)` reports
  `Error` for an out-of-range index;
- the batch owns its internal entries, but every `entryAt()` result and every
  `entry.result` access is a fresh independently owned projection;
- each `summary()` call returns a fresh owned `ScanSummaryView` derived once
  from Core's `ScanResults::summary()`; all public Core summary fields and
  `bySeverity()` are exposed;
- batches, entries, reports and summaries never retain source text. Only the
  caller-supplied IDs and Core reports are retained;
- batches, entries, results and summaries outlive the creating `ScanEngine` and
  the batch they came from. Free every object exactly once and never use an
  object after `free()`;
- reusing a `ScanResult` with `ScanEngine.explainFinding()` still requires a
  compatible engine, as with every existing result.

## Share Bundles

Share Bundles are available **only in the Flow configuration** (`transform`).
Core WASM exports no Share Bundle classes. Flow exposes the additive typed
builder and owned result surface below; scanning, transformation, source
ordering, validation and manifest construction all delegate to Core
`ShareBundleBuilder::scan_and_build_with_generated_at()`.

```ts
export class ShareBundleBuilder {
    constructor();
    modeRedact(): void;
    modeTemplate(): void;
    modePseudonymize(key: Uint8Array, prefix?: string | null,
        digest_bytes?: number | null): void;
    modeSynthesize(key: Uint8Array, marker?: string | null): void;
    scanAndBuild(engine: ScanEngine, sources: ScanBatchSource[]): ShareBundle;
    free(): void;
}

export class ShareBundle {
    readonly length: number;
    isEmpty(): boolean;
    entryAt(index: number): ShareBundleEntry;
    manifest(): ShareManifestView;
    summary(): ScanSummaryView;
    free(): void;
}

export class ShareBundleEntry {
    readonly id: string;
    readonly content: string;
    free(): void;
}

export class ShareManifestView {
    readonly mode: ShareModeKind;
    readonly generatedAtSeconds: bigint;
    readonly generatedAtNanoseconds: number;
    summary(): ScanSummaryView;
    free(): void;
}

export enum ShareModeKind {
    Redact = 0,
    Template = 1,
    Pseudonymize = 2,
    Synthesize = 3,
    Unknown = 4,
}
```

`ScanBatchSource` is reused: each input is `{ readonly id: string; readonly
source: string }`. Empty, duplicate and Unicode IDs are preserved by Core;
input order, clean sources, finding spans, candidates and UTF-8 byte lengths
retain Core semantics. Ambiguous candidates remain review-only and are never
transformed as findings.

```js
const engine = new ScanEngine();
const builder = new ShareBundleBuilder();
builder.modePseudonymize(crypto.getRandomValues(new Uint8Array(32)), "pseudo_", 16);
const bundle = builder.scanAndBuild(engine, [
  { id: "prod.env", source: "TOKEN=ghp_..." },
  { id: "notes.txt", source: "ordinary text" },
]);
try {
  for (let index = 0; index < bundle.length; index += 1) {
    const entry = bundle.entryAt(index); // fresh owned entry
    try {
      console.log(entry.id, entry.content);
    } finally {
      entry.free();
    }
  }
  const manifest = bundle.manifest(); // fresh owned manifest
  const summary = manifest.summary(); // fresh owned summary
  try {
    console.log(manifest.mode, manifest.generatedAtSeconds,
      manifest.generatedAtNanoseconds, summary.totalFindings);
  } finally {
    summary.free();
    manifest.free();
  }
} finally {
  bundle.free();
  builder.free();
  engine.free();
}
```

Configuration and failure semantics:

- no mode is selected by default, preserving Core `MissingShareMode`;
- the most recently selected mode wins; invalid key validation fails before a
  mode is staged and does not change an already selected mode;
- `modeTemplate()` uses Core's default Share Bundle template policy because the
  Core `ShareMode` contract exposes no template options for bundles;
- keyed modes require exactly 32 key bytes. The caller's `Uint8Array` is copied
  into Core options and is not retained by the adapter;
- omitted `prefix`, `digest_bytes` and `marker` retain Core defaults. Explicit
  empty strings are preserved. Core clamps pseudonym digest lengths to its
  supported range;
- Core transformation, source-length and missing-mode failures retain their
  existing source-free diagnostics as native JavaScript `Error` objects;
- structural `sources` inputs are validated before Core and never echo source
  text. `entryAt()` fails closed for an out-of-range index.

Timestamp handling is explicit. `wasm32-unknown-unknown` does not implement
`SystemTime::now()`, so the adapter reads the JavaScript host clock once through
`Date.now()` at the invocation boundary, converts the non-negative safe-integer
milliseconds to `SystemTime`, and passes it to Core's additive
`scan_and_build_with_generated_at()` entry point. Core remains the sole owner of
manifest mode, summary and timestamp storage; the adapter does not construct
manifest fields itself. `generatedAtSeconds` is a bigint and
`generatedAtNanoseconds` is always below one billion. Tests compare
deterministic manifest fields and validate the timestamp against the invocation
window rather than expecting independently generated timestamps to match.

Ownership boundaries:

- the builder owns only Core mode configuration, including keyed options;
- input IDs/sources are borrowed for one `scanAndBuild()` call and original
  source text is not retained;
- the bundle owns cloned IDs, Core-transformed content and Core manifest
  metadata;
- every `entryAt()`, `manifest()` and `summary()` call returns a fresh owned
  projection;
- entries, manifests and summaries remain valid after the builder, engine and
  bundle are freed. Free each object exactly once and never use it after
  `free()`.

The WASM surface intentionally exposes only Core's atomic `scan_and_build()`
workflow. It does not expose precomputed `ScanResults` build, arbitrary
timestamps from JavaScript, archives, serialization, filesystem output or
custom bundle template options. No transport, worker, parallel path or new
runtime dependency is involved.

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

A third direct-Core oracle generates complete ordered batch results and
Core-derived summaries for eight fixtures, including an empty batch, empty and
Unicode sources, duplicate/empty/Unicode IDs, built-ins, opt-in catalogs and all
custom rule families. Both generated variants compare entry order, IDs, UTF-8
byte lengths, findings, candidates, explanations and every public summary field
for 20 entries per variant. Runtime checks also compare every batched source
with `scan()`, exercise repeated batches, invalid structural inputs and
out-of-range indexes, then free the engine and batch before reusing independently
owned entries, reports and summaries. Flow additionally compares all six
transforms. The adapter remains synchronous and adds no runtime dependency or
parallel execution.

A fourth direct-Core oracle exercises the Flow-only Share Bundle surface with
11 source collections: empty, single and multiple sources; clean, finding and
candidate-only inputs; Unicode IDs/content; empty and duplicate IDs; all four
modes; keyed defaults and explicit configurations; and Core's missing-mode
failure. Both originals and transformed content are compared with a direct-Core
`ShareBundleBuilder` invocation, along with manifest mode and every summary
field. Timestamps are checked against the invocation window, not compared
between independent runs. Runtime checks cover repeated construction, mode
precedence, invalid keys and structural inputs, out-of-range entries, candidate
review semantics, and lifetimes after freeing the builder, engine and bundle.
Core WASM is checked for complete absence of the five Share Bundle exports.

The custom-remediation oracle qualifies all four families and seven concrete
actions plus every no-remediation form: 44 configurations and 132 complete
scan/stream/incremental reports per variant/runtime. Generated TypeScript pins
the additive signatures, and runtime tests preserve the old methods, check
unstaged invalid patterns/Unknown, and exercise empty IDs/matchers, duplicates
and collisions with each catalog. No runtime dependency is added.

The metadata/construction oracle also compares 16 empty/default/custom/catalog
configurations and 14 Core construction failures per variant/runtime. It pins
all five metadata fields and order, verifies UTF-8 IDs and ownership after engine
free, and checks native `Error` compatibility, exact existing messages, symbolic
categories and optional IDs. Native tests cover rare capture/capacity conversions;
automaton failure remains mapped without forcing resource exhaustion.

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

## Published package versus current source

The published `cribra-wasm` **0.4.7** registry release depends on Core `^0.6`.
The current workspace source also declares version `0.4.7` but depends on Core
`^0.7` and adds streaming, incremental, batch, scanner configuration/metadata
and Flow Share Bundle APIs. Those additions are not part of the published
package, and the same version cannot be republished; publishing them requires a
new adapter version.

## Usage

The following selects the published adapter and its published Core contract:

```toml
[dependencies]
cribra-wasm = "0.4.7"
```

For the newer session, batch, configuration and Share Bundle APIs, build this
workspace against Core 0.7.x. See
[Compatibility](../../docs/COMPATIBILITY.md) for the registry contract and the
source/local distinction.

The crate targets wasm32-unknown-unknown and is intended to be processed with
wasm-bindgen.

See the repository
[WASM interoperability documentation](https://github.com/Metalymph/cribra/blob/main/docs/WASM_INTEROP.md)
for the complete contract, semantic parity policy, browser validation, and
production build guidance.

## Historical: 0.4.6 compatibility release

This records the earlier published compatibility release; it is not current
installation guidance.

Version 0.4.6 depends on published `cribra ^0.5` (core 0.5.0). It preserves
the existing JavaScript/WASM API and whole-source behavior. No streaming
bindings, new features or third-party dependency upgrades are introduced.
Rust 1.97 remains the MSRV.

Publication is owned by the existing `Release WASM` workflow, triggered by
pushing `cribra-wasm-v0.4.6` at the qualified main commit. Core tag `v0.5.0`
is independent and must remain unchanged.

## License

MIT
