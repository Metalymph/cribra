# cribra-typescript

Provisional, unpublished TypeScript package for the Cribra WebAssembly scanner.

> **Status:** W3.2C-2 release readiness. Flow transformations, Share Bundles,
> Node ESM, browsers, and production bundlers are qualified from actual
> `npm pack` tarballs installed into isolated consumers.
>
> - The working manifest is `cribra-typescript@0.1.0`, marked
>   `"private": true`. The approved release name is `@metalymph/cribra`, but the
>   literal rename is deferred until authenticated npm ownership is verified.
>   Release mode fails closed while `cribraRelease.state` is
>   `"pending-ownership"`.
> - The package is **not published** and no registry record exists. Release
>   readiness and staging both fail closed while the identity is provisional;
>   local dry runs use `--allow-provisional`. Staging additionally requires an
>   approved identity and explicit, separately protected authorization.
> - `0.1.0` is the initial independent SemVer for this package. It is not tied to
>   the Core crate or `cribra-wasm` versions.
> - Transformations are **Flow only** (`scanner.transform`) and Share Bundles are
>   **Flow only** (`createShareBundle`). Core rejects both with
>   `CribraError("unsupported_capability")`; there is no fallback.
> - Node.js 22, 24, and 26 are qualified. The `>=22` engine range is a minimum
>   compatibility declaration, **not** evidence for odd-numbered or untested
>   Node releases.
> - Chrome, Playwright Chromium, Playwright Firefox, and Playwright WebKit are
>   qualified from the installed package. Playwright WebKit is **not** native
>   Safari.
> - Native Safari remote automation is not enabled on the qualification host, so
>   native Safari remains **unverified**.
> - Vite 8.3.6 (development and production) and webpack 5.112.2 (production) are
>   qualified with the package imported by name.
> - React Native and concurrent multi-threaded WASM remain outside the tested
>   matrix.

## What is implemented

- ESM-only package with TypeScript declarations.
- Side-effect-free import: importing the package does not load or instantiate
  WASM.
- Explicit `init()` that is idempotent and concurrency-safe.
- Synchronous `createScanner()`.
- Single-source `scanner.scan()`.
- Sequential `scanner.scanBatch()`.
- Byte-chunk `scanner.stream()` sessions.
- Revisioned `scanner.incremental()` documents.
- Scan-coupled `scanner.transform()` for redaction, templating,
  pseudonymization, and synthesis.
- Core-backed `createShareBundle()` for ordered source collections.
- Plain, serializable scan DTOs, incremental snapshots, transform results, and
  Share Bundle DTOs.
- `getCapabilities()` and `getVersion()`.
- Structured `CribraError` with incremental terminal classification.
- `byteSpanToTextRange()`.
- Deterministic scanner and session `dispose()`.

Public runtime exports:

```text
init
isInitialized
createScanner
createShareBundle
getCapabilities
getVersion
CribraError
byteSpanToTextRange
```

No generated `wasm-bindgen` class, numeric enum, or native handle appears in the
public declarations or in returned values.

## Requirements

### Consumers

- Node.js 22, 24, or 26 for the Node ESM entry.
- A browser with WebAssembly support for the browser entry.
- No Rust toolchain, Cargo, or `wasm-bindgen` installation. Those are build-time
  producer requirements only.
- No runtime npm dependencies. The installed package contains only `dist/`,
  `README.md`, and `LICENSE`.

### Building from source

- the Rust `wasm32-unknown-unknown` target;
- a `wasm-bindgen` CLI whose version exactly matches `Cargo.lock`;
- a Node package manager (`npm` is used here).

The installed `wasm-bindgen` version is checked before every WASM build. A
mismatch fails the build instead of producing a mismatched artifact.

## Build and test

```sh
cd packages/cribra
npm ci
npm run check        # builds the Flow bindings for types, then tsc --noEmit
npm run build        # Flow profile -> dist/
npm run test:types   # public declaration surface
npm test             # Node ESM tests against dist/
npm run package:check
npm run package:check:release  # fails closed while identity is provisional
npm run qualify      # Core + Flow: build, type check, Node, package, browser
```

`npm run qualify` writes profile-specific artifacts under
`target/typescript-qualification/{core,flow}/`. The default `dist/` build is the
Flow profile.

`npm pack --dry-run --json` is exercised by `npm run package:check`. The package
contains `dist/`, including `dist/wasm/cribra_bg.wasm`; it never contains
`src/`, `test/`, `scripts/`, `qualification/`, source maps, or qualification
tooling.

## Installed-artifact qualification

The private `qualification/` workspace installs the qualification toolchain and
tests only packed tarballs. It never imports repository `src/`, `dist/`, or a
workspace alias as a substitute.

```sh
cd packages/cribra/qualification
npm ci
npx playwright install chromium firefox webkit

cd ..
npm run qualify:pack       # Flow + test-only Core tarballs, reproducible build
npm run qualify:consumers  # Node, TypeScript, browsers, Vite, webpack, Safari
npm run qualify:all        # pack + consumers, including repeat-build check
```

Artifacts and evidence live under
`target/typescript-qualification/{artifacts,evidence}/`. Tarball names derive
from `package.json`. For the current manifest the Flow tarball is
`flow/cribra-typescript-0.1.0.tgz`; the secondary test-only Core tarball is
`core/cribra-typescript-0.1.0-core.tgz`. Their evidence is recorded separately
and only the Flow tarball may ever become public.

Qualified matrix for W3.2C-1:

| Area | Qualified |
| --- | --- |
| Node ESM | Node 22.22.0, 24.22.2, 26.13.0; default asset plus bytes, ArrayBuffer, `file:` URL, path, and HTTP URL initialization; Flow and test-only Core |
| TypeScript | 5.9.3 and 7.0.2 with `NodeNext` and `Bundler` resolution; declaration closure checked |
| Browser (installed package) | Google Chrome 156.0.8078.74, Playwright Chromium 156.0.8078.4, Playwright Firefox 157.0, Playwright WebKit 27.2 |
| Bundlers (package-name import) | Vite 8.3.6 dev + production; webpack 5.112.2 production |
| WASM delivery | emitted hashed URL, `application/wasm` response, real scan/transform/Share Bundle work in the browser |
| Native Safari | **Unverified**: remote automation was not enabled; the runner never enables it or changes security settings |

Playwright Firefox 157 needs a private `CFFIXED_USER_HOME` on macOS 27 because
Firefox cannot locate its profile under the TCC-protected application-support
directory. The qualification runner supplies a system-temp home; it does not
change host settings.

Interactive consumers always have the fallback of passing explicit WASM bytes
to `init({ wasm })`. Vite's default dependency optimization worked in this
qualification; `optimizeDeps.exclude` with the installed package name is
documented as a consumer-side workaround if a future Vite release changes that
behavior, never as manual asset copying.

## Release readiness and staging

Package identity is centralized in `package.json`:

- `name` and `version` are the only source of the package specifier and tarball
  names. Qualification, packing, test generation, and release scripts derive
  from them.
- `cribraRelease.approvedName` records the reviewed release name
  (`@metalymph/cribra`). `cribraRelease.state` is `"pending-ownership"` until
  authenticated npm ownership is proven.
- Release mode requires `state: "approved"`, `name === approvedName`,
  `private` absent, `publishConfig.access: "public"`,
  `publishConfig.provenance: true`, and the npmjs registry.

Local release checks:

```sh
npm run release:readiness -- --local --skip-qualification --allow-provisional
npm run release:verify -- \
  --release-root ../../target/typescript-release --provisional
```

The workflow `.github/workflows/typescript-release.yml` has two manual paths:

1. `readiness` reuses the full `typescript.yml` qualification matrix, verifies
   the dispatched tag and commit, requires byte-for-byte repeat-pack evidence
   and the complete consumer matrix, then uploads
   `typescript-release-<tag>` containing the manifest, `SHA256SUMS`, and the
   qualified tarball. It has `contents: read` and no OIDC permission.
2. `stage` runs only after the protected `npm-release` environment approves. It
   requires `CRIBRA_RELEASE_AUTHORIZED=1`, an exact `stage <tag>` confirmation,
   an approved identity, the qualified commit/ref, the recorded tarball digest,
   and proof that the version is not already published. It uses
   `id-token: write` and `npm stage publish`, then stops. A maintainer must
   download the staged tarball, compare its SHA-256 with the manifest, and
   approve with 2FA. The workflow never approves or promotes the staged version.

See [the release runbook](../../docs/TYPESCRIPT_RELEASE.md) for the ownership
proof, first-release bootstrap, and rollback limits.

## Initialization

```js
import { init, createScanner } from "cribra-typescript";

await init();
const scanner = createScanner();
const result = scanner.scan("token=...");
```

- `init()` shares one in-flight promise between concurrent callers.
- A resolved initialization is terminal for the module instance.
- A load or instantiation rejection is retryable **only** while the generated
  glue has not published a WASM export table. The glue assigns that table after
  successful finalization, so a rejected load leaves no partially active
  module.
- If the glue resolves but wrapper metadata validation fails, the WASM module is
  already active. The wrapper records a fatal `wasm_instantiate_failed` state
  instead of pretending initialization never happened; later `init()` calls and
  scanner operations reject with that structured error.
- `createScanner()`, `getCapabilities()`, and `getVersion()` throw
  `CribraError("not_initialized")` before successful initialization. They never
  auto-initialize.
- `byteSpanToTextRange()` has no native dependency and works before `init()`.

### Asset loading

| Input | Node ESM entry | Browser entry |
| --- | --- | --- |
| omitted | packaged `dist/wasm/cribra_bg.wasm` read with `node:fs/promises` | packaged WASM URL passed to the generated glue |
| `Uint8Array` / `ArrayBuffer` | passed to the generated glue | passed to the generated glue |
| `file:` URL or filesystem path | read with `node:fs/promises` | not applicable |
| `http(s):` URL | fetched by the Node loader | passed to the generated glue (`instantiateStreaming` and fallback) |
| other URL | passed to the generated glue | passed to the generated glue |

Only the Node entry imports `node:*`. The browser entry graph is checked for
static Node built-in imports.

The `browser` export condition selects `dist/index.browser.js`, which resolves
the packaged WASM through `new URL(..., import.meta.url)`. Vite and webpack are
qualified against that conditional export. Rollup directly, esbuild directly,
React Native, and native Safari are outside the tested matrix. Callers can
always pass explicit bytes to `init({ wasm })`.

## Scanner and result lifecycle

`createScanner(options)` is synchronous and compiles the selected built-in
catalog and custom rules once. `builtins` defaults to `"current"`; use
`"none"` for a custom-only scanner. `financialBuiltins` and `personalBuiltins`
are opt-in. Custom rules are appended after selected built-ins, and configuring a
custom rule never replaces the built-in catalog.

```js
const scanner = createScanner({
  builtins: "none",
  rules: [
    {
      kind: "literal",
      id: "acme.token",
      value: "acme_live_",
      severity: "critical",
      remediation: "rotate_credential",
    },
  ],
});

const result = scanner.scan(source, { includeExplanations: true });
scanner.dispose();
```

- `scan` and `scanBatch` return plain frozen DTOs. They contain no native handle,
  no source text, and no matched value.
- Results remain valid after `scanner.dispose()`.
- `dispose()` is idempotent. Scanning after disposal throws
  `CribraError("disposed")`.
- Rule metadata remains readable after disposal.
- Finding explanations are opt-in through `includeExplanations`. Candidate
  explanations are always present for review candidates.
- `scanBatch` uses one native batch call, preserves input order, and preserves
  empty, Unicode, and duplicate source IDs.
- UTF-8 byte offsets are preserved exactly. `Finding.span` and `Candidate.span`
  are half-open UTF-8 byte ranges; `line` and `column` are one-based and columns
  count Unicode scalar values.

## Streaming

```js
const session = scanner.stream({ limits: { maxCandidates: 10_000 } });
session.push(new TextEncoder().encode("first chunk"));
session.push(new Uint8Array());
session.push(new TextEncoder().encode("second chunk"));
const result = session.finish();
session.dispose();
```

- `push()` accepts `Uint8Array` chunks in source order. UTF-8 scalars and CRLF
  pairs may cross chunk boundaries. Empty active pushes are no-ops.
- The caller's buffer is copied into Core and never retained. There is no
  pre-EOF or partial-report API.
- `finish()` validates genuine EOF, completes pending work, and returns one
  complete detached `ScanResult`. Incomplete UTF-8 at EOF fails without a
  partial result.
- Streaming admission is per ruleset. An unsupported ruleset fails at
  `stream()` with `stream_unsupported_ruleset`. The default built-in catalog is
  currently **not** streamable; use an admitted custom ruleset. There is no
  whole-source fallback and no source-string reconstruction.
- A native push or finalization failure is terminal and releases the stream.
  Later `push()` or `finish()` calls throw `stream_not_active`.
  - A successful `finish()` also consumes the stream. `disposed` is `true`
    after success, terminal failure, or explicit disposal.
  - Explicit `dispose()` discards partial state; later calls throw
    `CribraError("disposed")` with domain `"stream"`.
- `StreamLimits` accepts independent optional `maxCandidates`,
  `maxStreamStorageBytes`, and `maxRetainedResults`. Missing means unlimited;
  explicit `0` is a real limit.
- A stream session owns its Core state and remains usable after its scanner is
  disposed. Disposing the session never disposes the scanner.

## Incremental scanning

```js
const session = scanner.incremental("initial text");
let revision = session.snapshot().revision; // 0n

revision = session.applyEdit(revision, 0, 0, "inserted ");
revision = session.applyEdit(revision, 8, 8, "");
const snapshot = session.snapshot();
session.dispose();
```

- Edits replace the half-open **UTF-8 byte range** `[start, end)`. There is no
  implicit UTF-16 index conversion.
- `expectedRevision` must be an exact `bigint` in the `u64` range and must equal
  the committed revision. A successful edit, including a no-op, returns the
  next `bigint` revision.
- Insertion is `start === end`; deletion uses an empty replacement.
- Invalid revisions, offsets, range order, and non-scalar boundaries are
  rejected. Recoverable input errors leave the committed document and revision
  unchanged.
- Resource or invariant failures are terminal: `isActive` becomes `false` and
  future edits fail, but `snapshot()` continues to expose the last committed
  state until explicit disposal.
- `dispose()` is idempotent and frees the document. Snapshots already returned
  remain valid. After disposal, edits and snapshots throw
  `CribraError("disposed")` with domain `"incremental"`.
- `IncrementalSnapshot` is a plain DTO containing `revision: bigint` and a
  detached `ScanResult`. `JSON.stringify(snapshot)` throws because `bigint` has
  no JSON representation; convert the revision explicitly (for example with
  `revision.toString()`) before serializing.
- `IncrementalLimits` accepts independent optional `maxCandidates`,
  `maxRetainedCacheRecords`, `maxMaterializedResults`, and
  `maxTransactionStorageBytes`. Missing means unlimited; explicit `0` is a real
  limit.
- An incremental session owns its Core document and remains usable after its
  scanner is disposed. Disposing the session never disposes the scanner.

### Custom rule limitation

`PatternRuleSpec.captureGroup` exists in the approved type contract, but the
current WASM adapter exposes no capture-aware builder entry and Core's
captured-pattern constructor is private. Supplying `captureGroup` therefore
throws `CribraError("unsupported_capability")`. No Core or adapter change is made
in this milestone.

## Transformations (Flow only)

`scanner.transform(source, request)` scans and transforms one source atomically.
It performs exactly **one** native scan per call and returns both the scan and
the transformed output:

```js
const result = scanner.transform("TOKEN=acme_live_...", { kind: "redact" });
result.scan;   // detached ScanResult for the same scan
result.output; // "[REDACTED]" transformation, candidates are never rewritten
```

There are four modes:

| `kind` | Options | Core default |
| --- | --- | --- |
| `"redact"` | `replacement?` | `"[REDACTED]"` |
| `"template"` | `namespace?`, `numbered?` | `"CRIBRA"`, `false` |
| `"pseudonymize"` | `key` (exactly 32 bytes), `prefix?`, `digestBytes?` | `"cribra_pseudo_"`, `16` (Core clamps to 8..=32) |
| `"synthesize"` | `key` (exactly 32 bytes), `marker?` | `"cribra_synthetic"` |

Semantics and limits:

- The transform is **scan-coupled** because the WASM adapter can only transform
  against a live native result. Each call releases its native result; there is
  no rescan, no stored-DTO transformation, and no TypeScript matching,
  replacement, or synthesis.
- Transformation only rewrites **findings**. Review candidates are never
  modified.
- Keyed requests require a `Uint8Array` of exactly 32 bytes. The key is copied
  for the call and Core does not retain it. JavaScript cannot promise secure
  erasure of the caller's buffer; zeroing it afterwards is best-effort hygiene,
  not a security guarantee.
- The complete request is validated before any scan. Invalid requests throw
  `invalid_argument` (or `invalid_key_length`) with domain `"transform"` and
  perform no native scan.
- Overlapping findings in template, pseudonymization, or synthesis mode fail with
  `transform_overlapping_spans`. Redaction tolerates overlaps.
- `TransformResult` and its `scan` are frozen plain DTOs containing no native
  handle. They remain valid after `scanner.dispose()`.
- Core rejects `transform` with `CribraError("unsupported_capability")`
  (domain `"capability"`) before touching native state. There is no fallback.

## Share Bundles (Flow only)

`createShareBundle(scanner, sources, options)` builds an ordered collection
through Core's atomic native Share Bundle builder:

```js
const bundle = createShareBundle(
  scanner,
  [
    { id: "a.txt", source: "TOKEN=acme_live_..." },
    { id: "b.txt", source: "clean" },
  ],
  {
    mode: { kind: "pseudonymize", key: new Uint8Array(32) },
    generatedAt: new Date(1_700_000_000_123),
  },
);
```

`ShareBundleOptions.mode` accepts the same four discriminated modes as
transformations. `generatedAt` is optional:

- omitted: the adapter reads the JavaScript host clock once at the invocation
  boundary and passes it to Core; the manifest still owns the value;
- an explicit `Date`: its milliseconds are passed through the Core-backed
  `scanAndBuildWithGeneratedAt` method, so Core, not the wrapper, sets the
  manifest timestamp. An invalid date fails with `share_timestamp_invalid`.

Returned DTOs:

```ts
interface ShareBundle {
  readonly entries: readonly ShareBundleEntry[]; // { id, content }, input order
  readonly manifest: ShareManifest;              // mode, seconds, nanos, summary
  readonly summary: ScanSummary;
}
```

- `entries` preserve input order, including empty, duplicate, and Unicode IDs.
- `manifest.generatedAtSeconds` is a safe `number`; the adapter converts Core's
  `bigint` only after validating `0 <= seconds <= Number.MAX_SAFE_INTEGER` and
  `0 <= nanoseconds < 1_000_000_000`. Out-of-range values fail with
  `share_timestamp_invalid`.
- `manifest.summary` and `summary` are independent detached projections of
  Core's summary.
- The native builder, bundle, entries, manifest, and summary views are freed
  deterministically in `finally` blocks. The returned objects contain no native
  handle and remain valid after `scanner.dispose()`.
- The scanner registry is a private `WeakMap`. A disposed scanner is rejected
  before any native builder or handle is created; a foreign object is rejected
  with `invalid_argument` (domain `"share"`).
- Core rejects `createShareBundle` with `CribraError("unsupported_capability")`
  (domain `"capability"`) before creating a native builder. There is no fallback.



`getCapabilities()` copies the manifest from the **actually loaded WASM
artifact**. Values are not masked based on wrapper progress.

During W3.2B-4:

- `streaming`, `incremental`, `ruleMetadata`, `explanations`, `batchScan`, and
  `singleSourceScan` are `true` for both profiles because the adapter exports
  those operations and this wrapper exposes them.
- `redaction`, `templating`, `pseudonymization`, `synthesis`, and `shareBundle`
  follow the compiled profile (`false` for Core, `true` for Flow). The wrapper
  now exposes the matching Flow methods and rejects them explicitly on Core.
- `parallel` is `false`.
- `utf16OffsetConversion` is a wrapper capability and is always `true`.

`streaming: true` describes the backend, not a particular scanner's ruleset.
`stream()` can still reject a ruleset with `stream_unsupported_ruleset`.

`getVersion()` returns `{ package, wasmAdapter }`. `core` is absent because the
current backend cannot prove a Core version, and the adapter version is never
substituted for it.

## Offsets

`byteSpanToTextRange(source, span)` converts a UTF-8 byte span into a half-open
UTF-16 code-unit range suitable for `String.prototype.slice`. It validates in
this order:

1. unpaired UTF-16 surrogate in `source` → `invalid_unicode_string`;
2. non-safe-integer or negative endpoint → `invalid_span_value`;
3. `start > end` → `invalid_span_order`;
4. `end` beyond the UTF-8 byte length → `span_out_of_bounds`;
5. endpoint inside a UTF-8 scalar → `span_not_utf8_boundary`.

It never clamps, rounds, or substitutes U+FFFD.

## Error model

`CribraError` is the only public error type. It exposes `name`, `code`,
`domain`, an optional `ruleId`, and an optional incremental `terminal` boolean.
Messages are fixed and never interpolate source text, replacement text, matched
values, matcher payloads, or key material. Raw backend errors are normalized
before crossing the public API. `cause` is omitted in this milestone because
adapter diagnostics cannot be proven source-free.

Adapter scanner-construction categories map through a static table to stable
codes such as `invalid_pattern`, `pattern_matches_empty`, and
`duplicate_rule_id`. Streaming codes map to `stream_*`; incremental codes map to
`incremental_*` and preserve the native terminal classification; adapter
transform failures map to `transform_*`; adapter Share Bundle failures map to
`share_*`. Unknown adapter outcomes map to `transform_unknown` or `share_unknown`
and never leak raw native diagnostics. No numeric `wasm-bindgen` enum or
generated error class is exposed, and generated error objects are freed exactly
once during normalization.

## Browser and bundler qualification

The legacy Chrome smoke test in `npm run qualify`:

- starts a local Node HTTP server with `application/wasm`;
- launches Chrome/Chromium headless (`CHROME_BIN` or a common installation
  path);
- checks default URL loading, concurrent init, capabilities, default and custom
  scanners, batch scanning, streaming across UTF-8/CRLF boundaries, revisioned
  incremental edits, Flow transformations, Flow Share Bundles, Core rejection of
  both, offsets, disposal, and result independence;
- runs for both Core and Flow profiles;
- fails clearly when no browser is found, the browser exits non-zero, times out,
  or does not produce a success marker;
- always closes the HTTP server.

The W3.2C-1 installed-artifact matrix extends this with Playwright Chromium,
Playwright Firefox, and Playwright WebKit, plus Vite and webpack production
bundles that import `cribra-typescript` by package name. See
[Installed-artifact qualification](#installed-artifact-qualification).

Native Safari is **not claimed**. The qualification host does not have Safari
remote automation enabled, and the runner never enables it or changes security
settings. Playwright WebKit is reported separately and is not Safari evidence.

## License

MIT. See `LICENSE`.
