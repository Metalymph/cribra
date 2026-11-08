---
title: "TypeScript quick start"
description: "Initialize the provisional cribra-typescript WebAssembly package, scan caller-owned text, and convert UTF-8 byte spans."
order: 16
---

# TypeScript quick start

> **Status:** TypeScript is implemented against the Cribra WebAssembly backend as
> the provisional, **unpublished** package `cribra-typescript@0.1.0`. The working
> manifest is marked `"private": true` and `cribraRelease.state` is
> `"pending-ownership"`. The project has not published this package, and no npm
> registry record exists. `@metalymph/cribra` is the intended future name; it is
> not currently an available installation instruction, and authenticated npm
> ownership has not been established.

The package is ESM-only, has no runtime npm dependencies, and performs no WASM
work when imported. `await init()` loads the packaged WebAssembly asset; every
other public entry point is synchronous. Transformations and Share Bundles are
**Flow only** and are covered in the [sessions and transformations
guide](/docs/typescript-sessions-and-transformations/). Runtime qualification is
covered separately in the [runtime compatibility
guide](/docs/typescript-runtime-compatibility/).

## Build and install a local package

The package is not available from a registry. Build it from a source checkout or
install a locally packed tarball. The qualified Flow tarball is produced by the
same installed-consumer machinery used by the repository:

```sh title="repository checkout"
cd packages/cribra
npm ci
npm run build
npm pack
```

`npm pack` writes `cribra-typescript-0.1.0.tgz` in `packages/cribra/`. To produce
the release-qualified Flow tarball instead, run `npm run qualify:pack`; it writes
`target/typescript-qualification/artifacts/flow/cribra-typescript-0.1.0.tgz`.
That command requires the Rust WASM target, a matching `wasm-bindgen` CLI, and
Binaryen. The adjacent Core tarball is test-only and must not be distributed.

Install the local tarball into your application:

```sh title="application"
npm install --ignore-scripts --no-audit --no-fund \
  /path/to/cribra/target/typescript-qualification/artifacts/flow/cribra-typescript-0.1.0.tgz
```

Import the package by its provisional name, `cribra-typescript`. Do not replace
the local path with an `@metalymph/cribra` registry installation.

## Initialize the backend

```ts title="initialize.ts"
import {
  createScanner,
  getCapabilities,
  getVersion,
  init,
  isInitialized,
} from "cribra-typescript";

console.log(isInitialized()); // false — import performs no WASM work

await init();

console.log(isInitialized()); // true
const capabilities = getCapabilities();
const version = getVersion();

console.log(capabilities.backend); // "wasm"
console.log(capabilities.profile); // "flow"
console.log(version.package); // "0.1.0"

// Creating a scanner before init() throws CribraError("not_initialized").
const scanner = createScanner({ builtins: "none" });
scanner.dispose();
```

`init()` is idempotent and concurrency-safe. With no argument it resolves the
WASM asset relative to the installed package. To supply an explicit asset, pass
`init({ wasm })`, where `wasm` is a `Uint8Array`, `ArrayBuffer`, `URL`, or string
path supported by the current runtime:

```ts
await init({ wasm: new URL("./cribra_bg.wasm", import.meta.url) });
```

`createScanner()`, `scan()`, and the session factories require a successfully
initialized backend. `getCapabilities()` and `getVersion()` also throw
`CribraError("not_initialized")` before initialization.

## Scan caller-owned text

The following program uses synthetic rules and input. A **finding** is an
authoritative classification; a review-only candidate is not a finding and is
never transformed automatically.

```ts title="scan.ts"
import { createScanner, init } from "cribra-typescript";
import type { RuleSpec } from "cribra-typescript";

await init();

const rules: readonly RuleSpec[] = [
  {
    id: "demo.api-key",
    kind: "prefix",
    value: "demo_live_",
    severity: "critical",
    remediation: "rotate_credential",
  },
  {
    id: "demo.secret",
    kind: "literal",
    value: "demo-secret-123",
    severity: "high",
  },
];

const scanner = createScanner({ builtins: "none", rules });

const source = "ACME_API_KEY=demo_live_7f3a91\nnote=demo-secret-123\n";

try {
  const result = scanner.scan(source, { includeExplanations: true });

  console.log(result.findingCount);
  for (const finding of result.findings) {
    console.log(
      finding.ruleId,
      finding.span,       // { start, end } in UTF-8 bytes
      finding.line,       // one-based
      finding.column,     // one-based, Unicode scalar columns
      finding.severity,
    );
  }
} finally {
  scanner.dispose();
}
```

`ScanResult` contains counts, findings, candidates, and the UTF-8 source length.
It does **not** contain the source text or copies of matched values. Your
application owns the input, derived output, logging, retention, and sharing
decisions.

## Scan an ordered batch

`scanBatch()` preserves input order, duplicate IDs, and empty IDs. The aggregate
summary is Core-owned and is not recalculated in JavaScript.

```ts title="batch.ts"
import { createScanner, init } from "cribra-typescript";
import type { ScanSource } from "cribra-typescript";

await init();

const scanner = createScanner({
  builtins: "none",
  rules: [
    {
      id: "demo.secret",
      kind: "literal",
      value: "demo-secret-123",
      severity: "high",
    },
  ],
});

const sources: readonly ScanSource[] = [
  { id: "", source: "clean" },
  { id: "same", source: "demo-secret-123" },
  { id: "same", source: "no findings" },
];

try {
  const batch = scanner.scanBatch(sources);
  console.log(batch.entries.map((entry) => entry.id)); // ["", "same", "same"]
  console.log(batch.summary.scannedSources); // 3
  console.log(batch.summary.totalFindings); // 1
} finally {
  scanner.dispose();
}
```

## Convert UTF-8 byte spans to JavaScript ranges

Finding and candidate spans are zero-based, half-open **UTF-8 byte offsets**.
JavaScript string methods use **UTF-16 code units**. Use
`byteSpanToTextRange()` before calling `slice()`; do not use a byte offset
directly as a string index.

```ts title="offsets.ts"
import { byteSpanToTextRange, createScanner, init } from "cribra-typescript";

await init();

const scanner = createScanner({
  builtins: "none",
  rules: [
    {
      id: "demo.secret",
      kind: "literal",
      value: "demo-secret-123",
      severity: "high",
    },
  ],
});

const source = "🦀 demo-secret-123";

try {
  const result = scanner.scan(source);
  const finding = result.findings[0];
  if (finding === undefined) throw new Error("expected one finding");

  const range = byteSpanToTextRange(source, finding.span);
  console.log(source.slice(range.start, range.end)); // "demo-secret-123"
} finally {
  scanner.dispose();
}
```

The helper rejects non-integer or out-of-bounds spans, spans that split a UTF-8
scalar, and sources containing an unpaired UTF-16 surrogate. It throws a
`CribraError` with the `offset` domain.

## Handle errors and dispose resources

`scanner.dispose()` is idempotent. Scanning methods throw
`CribraError("disposed")` after disposal. Stream and incremental sessions own
their own state and remain usable after the creating scanner is disposed; dispose
them separately. Share Bundle data is detached and has no disposal method.

```ts title="errors-and-disposal.ts"
import { CribraError, createScanner, init } from "cribra-typescript";

await init();

const scanner = createScanner({
  builtins: "none",
  rules: [
    {
      id: "demo.secret",
      kind: "literal",
      value: "demo-secret-123",
      severity: "high",
    },
  ],
});

try {
  scanner.scan("demo-secret-123");
} finally {
  scanner.dispose();
}

try {
  scanner.scan("demo-secret-123");
} catch (error) {
  if (error instanceof CribraError) {
    console.log(error.code);   // "disposed"
    console.log(error.domain); // "scanner"
  }
}
```

Use `try/finally` for short-lived scanners. The [API
reference](/docs/typescript-api-reference/#errors) lists every error domain and
code, and the [sessions guide](/docs/typescript-sessions-and-transformations/)
covers session-specific state.

## Next steps

- [TypeScript API reference](/docs/typescript-api-reference/)
- [Sessions and transformations](/docs/typescript-sessions-and-transformations/)
- [Runtime compatibility](/docs/typescript-runtime-compatibility/)
- [WebAssembly integration](/docs/integration-wasm/)
