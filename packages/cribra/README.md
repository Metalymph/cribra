# cribra-typescript

Provisional, unpublished TypeScript foundation for the Cribra WebAssembly
scanner.

> **Status:** W3.2B-2 package foundation.
>
> - Package identity `cribra-typescript` is temporary and the package is marked
>   `"private": true`. The final npm name is not approved.
> - This package wraps the merged WASM adapter. It is not published and is not
>   part of a release.
> - The wrapper is **not fully qualified** until W3.2B-4.
> - Streaming, incremental scanning, transformations, and Share Bundles are
>   **not implemented by this wrapper yet**, even when the loaded adapter reports
>   those backend capabilities as available.

## What is implemented

- ESM-only package with TypeScript declarations.
- Side-effect-free import: importing the package does not load or instantiate
  WASM.
- Explicit `init()` that is idempotent and concurrency-safe.
- Synchronous `createScanner()`.
- Single-source `scanner.scan()`.
- Sequential `scanner.scanBatch()`.
- Plain, serializable scan DTOs.
- `getCapabilities()` and `getVersion()`.
- Structured `CribraError`.
- `byteSpanToTextRange()`.
- Deterministic scanner `dispose()`.

Public runtime exports:

```text
init
isInitialized
createScanner
getCapabilities
getVersion
CribraError
byteSpanToTextRange
```

No generated `wasm-bindgen` class, numeric enum, or native handle appears in the
public declarations or in returned values.

## Requirements

- Node.js 20 or newer for the Node ESM entry.
- Chrome or Chromium for the browser smoke test.
- To build from source:
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
npm run qualify      # Core + Flow: build, type check, Node, package, browser
```

`npm run qualify` writes profile-specific artifacts under
`target/typescript-qualification/{core,flow}/`. The default `dist/` build is the
Flow profile.

`npm pack --dry-run --json` is exercised by `npm run package:check`. The package
contains `dist/`, including `dist/wasm/cribra_bg.wasm`; it never contains
`src/` or `test/`.

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

Bundler compatibility is **not claimed** in this milestone. Vite, webpack,
Rollup, esbuild, Safari, Firefox, and React Native are outside the tested matrix.
Callers can always pass explicit bytes to `init({ wasm })`.

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

### Custom rule limitation

`PatternRuleSpec.captureGroup` exists in the approved type contract, but the
current WASM adapter exposes no capture-aware builder entry and Core's
captured-pattern constructor is private. Supplying `captureGroup` therefore
throws `CribraError("unsupported_capability")`. No Core or adapter change is made
in this milestone.

## Capabilities versus wrapper availability

`getCapabilities()` copies the manifest from the **actually loaded WASM
artifact**. Values are not masked based on wrapper progress.

During W3.2B-2:

- `streaming`, `incremental`, `ruleMetadata`, `explanations`, `batchScan`, and
  `singleSourceScan` are `true` for both profiles because the adapter exports
  those operations.
- Transformation and Share Bundle flags follow the compiled profile (`false` for
  Core, `true` for Flow).
- `parallel` is `false`.
- `utf16OffsetConversion` is a wrapper capability and is always `true`.

A `true` flag can therefore mean **available in the backend but not yet exposed
as a TypeScript wrapper method**. The `Scanner` surface in this milestone
declares only `rulesCount`, `rules`, `disposed`, `scan`, `scanBatch`, and
`dispose`. Deferred methods are not declared yet.

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
`domain`, and an optional `ruleId`. Messages are fixed and never interpolate
source text, matched values, matcher payloads, or key material. Raw backend
errors are normalized before crossing the public API. `cause` is omitted in this
milestone because adapter diagnostics cannot be proven source-free.

Adapter scanner-construction categories map through a static table to stable
codes such as `invalid_pattern`, `pattern_matches_empty`, and
`duplicate_rule_id`. No numeric `wasm-bindgen` enum is exposed.

## Browser qualification

The browser smoke test:

- starts a local Node HTTP server with `application/wasm`;
- launches Chrome/Chromium headless (`CHROME_BIN` or a common installation
  path);
- checks default URL loading, concurrent init, capabilities, default and custom
  scanners, batch scanning, offsets, disposal, and result independence;
- runs for both Core and Flow profiles;
- fails clearly when no browser is found, the browser exits non-zero, times out,
  or does not produce a success marker;
- always closes the HTTP server.

Only Chrome/Chromium was tested. Safari and Firefox are not claimed.

## License

MIT. See `LICENSE`.
