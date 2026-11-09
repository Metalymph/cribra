---
title: "TypeScript sessions and transformations"
description: "Admitted byte streaming, revisioned incremental edits, the four Flow transformations, and Share Bundles."
order: 18
---

# TypeScript sessions and transformations

Streaming, incremental editing, transformations, and Share Bundles are explicit
APIs. A default scanner is not automatically streamable, and transformations are
**Flow only**. On a Core-profile backend, `scanner.transform()` and
`createShareBundle()` throw `CribraError("unsupported_capability")`; there is no
whole-source or untransformed fallback.

All examples below use synthetic input and keys.

## Streaming

`scanner.stream()` opens an append-only byte session. The session is admitted
only for compatible rulesets. Complete built-in portfolios are not automatically
admitted, so select a custom ruleset and handle admission errors rather than
silently falling back.

```ts title="stream.ts"
import { createScanner, init } from "@metalymph/cribra";

await init();

const scanner = createScanner({
  builtins: "none",
  rules: [
    {
      id: "demo.stream",
      kind: "literal",
      value: "demo-secret-123",
      severity: "high",
    },
  ],
});

const source = "prefix 🦀 demo-secret-123 suffix";
const bytes = new TextEncoder().encode(source);
const session = scanner.stream();

try {
  // Split inside the four-byte emoji scalar.
  const split = new TextEncoder().encode("prefix ").length + 1;
  session.push(bytes.subarray(0, split));
  session.push(bytes.subarray(split));

  const result = session.finish();
  console.log(result.findingCount); // 1
} finally {
  session.dispose();
  scanner.dispose();
}
```

`push()` copies each `Uint8Array`; the caller may reuse the buffer immediately.
Chunks may split UTF-8 scalars and CRLF pairs. `finish()` checks genuine EOF and
returns the complete detached report. An incomplete trailing scalar is an error,
not an implicitly repaired input.

Optional limits are logical budgets, not a process-wide heap cap:

```ts
const session = scanner.stream({
  limits: {
    maxCandidates: 10_000,
    maxStreamStorageBytes: 8 * 1024 * 1024,
    maxRetainedResults: 10_000,
  },
});
```

A stream failure is terminal. `finish()` consumes the session; create a new
session for another source. The session owns its state and remains usable after
`scanner.dispose()`.

## Incremental editing

`scanner.incremental(source)` owns a complete UTF-8 document. An edit replaces a
half-open UTF-8 byte range `[start, end)` and supplies the expected `bigint`
revision. Successful edits return the next revision and make a complete new
report available through `snapshot()`.

```ts title="incremental.ts"
import { createScanner, init } from "@metalymph/cribra";

await init();

const scanner = createScanner({
  builtins: "none",
  rules: [
    {
      id: "demo.incremental",
      kind: "literal",
      value: "demo-secret-123",
      severity: "high",
    },
  ],
});

const session = scanner.incremental("🦀 demo-secret-123\n");

try {
  let revision = 0n;

  const start = new TextEncoder().encode("🦀 ").length;
  const end = start + new TextEncoder().encode("demo-secret-123").length;

  revision = session.applyEdit(
    revision,
    start,
    end,
    "demo-secret-456",
  );

  const snapshot = session.snapshot();
  console.log(revision);                 // 1n
  console.log(snapshot.revision);        // 1n
  console.log(snapshot.result.findingCount); // 0
} finally {
  session.dispose();
  scanner.dispose();
}
```

The revision is `bigint` because Core revisions are `u64`; `number` would lose
precision. Endpoints must be valid UTF-8 scalar boundaries. A stale revision or
invalid range is recoverable: correct the request against the current committed
revision. Resource-budget and internal-invariant failures are terminal for
further edits, while the last committed snapshot remains readable.

Optional limits are independent of streaming limits:

```ts
const session = scanner.incremental("clean text", {
  limits: {
    maxCandidates: 10_000,
    maxRetainedCacheRecords: 20_000,
    maxMaterializedResults: 10_000,
    maxTransactionStorageBytes: 8 * 1024 * 1024,
  },
});
```

The session owns the entire document, so these budgets do not bound total
document retention. Disposing the creating scanner does not invalidate the
session.

## The four transformations

`scanner.transform(source, request)` performs exactly one native scan and returns
both the scan report and the transformed source. It does not accept a previously
stored report, and multiple transformations require separate calls.

```ts title="transformations.ts"
import { createScanner, init } from "@metalymph/cribra";
import type { TransformRequest } from "@metalymph/cribra";

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

const key = new Uint8Array(32).fill(0x2a);
const source = "🦀 demo-secret-123\n";

const requests: readonly TransformRequest[] = [
  { kind: "redact" },
  { kind: "template", namespace: "DEMO", numbered: true },
  { kind: "pseudonymize", key, prefix: "demo_pseudo_", digestBytes: 16 },
  { kind: "synthesize", key, marker: "demo_synthetic" },
];

try {
  for (const request of requests) {
    const result = scanner.transform(source, request);
    console.log(request.kind, result.output);
    console.log(result.scan.findingCount); // 1
  }
} finally {
  scanner.dispose();
}
```

| Request | Required fields | Optional fields and defaults |
| --- | --- | --- |
| `{ kind: "redact" }` | `kind` | `replacement` defaults to Core's `[REDACTED]`. |
| `{ kind: "template" }` | `kind` | `namespace` defaults to `CRIBRA`; `numbered` defaults to `false`. |
| `{ kind: "pseudonymize" }` | `kind`, `key` | `prefix` defaults to `cribra_pseudo_`; `digestBytes` defaults to 16 and is clamped to 8–32. |
| `{ kind: "synthesize" }` | `kind`, `key` | `marker` defaults to `cribra_synthetic`. |

Keys must be exactly 32 bytes. The same key produces deterministic pseudonymous
or synthetic output for the same source and configuration. Only classified
finding spans are rewritten. Review-only candidates are never transformed, and
overlapping finding spans are rejected.

## Share Bundles

`createShareBundle()` scans and transforms an ordered source collection
atomically. It returns detached entries, a manifest, and a summary; the native
Share Bundle builder is an implementation detail with no public lifecycle.

```ts title="share-bundle.ts"
import {
  createScanner,
  createShareBundle,
  init,
} from "@metalymph/cribra";
import type { ScanSource } from "@metalymph/cribra";

await init();

const scanner = createScanner({
  builtins: "none",
  rules: [
    {
      id: "demo.api-key",
      kind: "prefix",
      value: "demo_live_",
      severity: "critical",
      remediation: "rotate_credential",
    },
  ],
});

const sources: readonly ScanSource[] = [
  { id: "config.env", source: "ACME_API_KEY=demo_live_7f3a91\n" },
  { id: "notes.txt", source: "clean\n" },
];

try {
  const bundle = createShareBundle(scanner, sources, {
    mode: { kind: "redact" },
    generatedAt: new Date("2026-01-01T00:00:00Z"),
  });

  console.log(bundle.entries.map((entry) => entry.id));
  console.log(bundle.manifest.mode); // "redact"
  console.log(bundle.summary.scannedSources); // 2
} finally {
  scanner.dispose();
}
```

`ShareMode` supports the same four modes: `redact`, `template`, `pseudonymize`,
and `synthesize`. A keyed mode uses the same 32-byte input-only key rules as
`scanner.transform()`. `generatedAt` is optional; when omitted, the backend uses
its host clock. Input order, duplicate IDs, and empty IDs are preserved.

A Share Bundle is not an archive, persistence format, or anonymity guarantee.
Its entries contain transformed source text, so review-only candidates and
undetected sensitive values may remain.

## Related guides

- [Quick start](/docs/typescript/)
- [API reference](/docs/typescript-api-reference/)
- [Streaming scanning](/docs/streaming-scanning/)
- [Incremental scanning](/docs/incremental-scanning/)
- [Explicit transformations](/docs/transformations/)
