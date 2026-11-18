---
title: "TypeScript API reference"
description: "The eight runtime exports, public types, Scanner methods, DTOs, ownership, UTF-8 coordinates, errors, privacy, and disposal."
order: 17
---

# TypeScript API reference

This reference describes the public declarations shipped by
`@metalymph/cribra@0.1.0`. The package is approved but unpublished; see the
[quick start](/docs/typescript/) for local installation. No generated
`wasm-bindgen` class, numeric enum, native handle, or `free()` method appears in
the public declarations or in returned values.

## Runtime exports

Exactly eight values are exported at runtime:

| Export | Signature | Notes |
| --- | --- | --- |
| `init` | `(options?: InitOptions) => Promise<void>` | Idempotent and concurrency-safe. Rejects on fatal initialization failure. |
| `isInitialized` | `() => boolean` | `false` after import and before a successful `init()`. |
| `createScanner` | `(options?: ScannerOptions) => Scanner` | Synchronous. Requires an initialized backend. |
| `createShareBundle` | `(scanner: Scanner, sources: readonly ScanSource[], options: ShareBundleOptions) => ShareBundle` | Flow only. Scans and transforms the ordered collection atomically. |
| `getCapabilities` | `() => Capabilities` | Requires initialization. |
| `getVersion` | `() => VersionInfo` | Requires initialization. |
| `CribraError` | `class CribraError extends Error` | Structured public error with `code` and `domain`. |
| `byteSpanToTextRange` | `(source: string, span: ByteSpan) => TextRange` | Pure UTF-8-to-UTF-16 conversion. Does not require initialization. |

The following are type-only exports. They disappear from emitted JavaScript.

```ts
export type {
  Backend, BatchEntry, BatchResult, ByteSpan, Candidate,
  CandidateEvidence, CandidateKind, Capabilities, Confidence,
  CribraErrorCode, DetectionMode, ErrorDomain, Explanation, Finding,
  IncrementalLimits, IncrementalOptions, IncrementalSession,
  IncrementalSnapshot, InitOptions, LiteralRuleSpec, PatternRuleSpec,
  PrefixRuleSpec, Profile, PseudonymizeRequest, RedactRequest,
  Remediation, RuleKind, RuleMetadata, RuleSpec, ScanOptions, ScanResult,
  ScanSource, ScanSummary, Scanner, ScannerOptions, Severity, ShareBundle,
  ShareBundleEntry, ShareBundleOptions, ShareManifest, ShareMode,
  ShareModeKind, StreamLimits, StreamOptions, StreamSession,
  SuffixRuleSpec, SynthesizeRequest, TemplateRequest, TextRange,
  TransformRequest, TransformResult, VersionInfo,
};
```

## Initialization

```ts
interface InitOptions {
  /** Explicit WASM bytes or URL. Omitted: resolve relative to the package asset. */
  readonly wasm?: Uint8Array | ArrayBuffer | URL | string;
}

interface Capabilities {
  readonly backend: "wasm" | "native";
  readonly profile: "core" | "flow";

  readonly singleSourceScan: boolean;
  readonly batchScan: boolean;
  readonly streaming: boolean;
  readonly incremental: boolean;

  readonly redaction: boolean;
  readonly templating: boolean;
  readonly pseudonymization: boolean;
  readonly synthesis: boolean;
  readonly shareBundle: boolean;

  readonly ruleMetadata: boolean;
  readonly explanations: boolean;
  readonly parallel: boolean;

  /** Wrapper-level helper; always available. */
  readonly utf16OffsetConversion: boolean;
}

interface VersionInfo {
  readonly package: string;
  readonly core?: string;
  readonly wasmAdapter?: string;
}
```

The installed Flow package reports `backend: "wasm"`, `profile: "flow"`, and
`parallel: false`. The test-only Core build reports its own capability set.
Transformations and Share Bundles are unavailable when `shareBundle` or the
corresponding transform capability is `false`.

## Scanner

```ts
interface Scanner {
  readonly rulesCount: number;
  readonly rules: readonly RuleMetadata[];
  readonly disposed: boolean;

  scan(source: string, options?: ScanOptions): ScanResult;
  scanBatch(sources: readonly ScanSource[], options?: ScanOptions): BatchResult;

  stream(options?: StreamOptions): StreamSession;
  incremental(source: string, options?: IncrementalOptions): IncrementalSession;

  transform(source: string, request: TransformRequest): TransformResult;

  dispose(): void;
}
```

`rulesCount` and `rules` expose the compiled ruleset. `scan`, `scanBatch`,
`stream`, `incremental`, and `transform` throw `CribraError("disposed")` after
disposal. `dispose()` is idempotent. Stream and incremental sessions outlive the
creating scanner.

### Scanner options

```ts
interface ScannerOptions {
  /** Defaults to "current". "none" selects an empty built-in catalog. */
  readonly builtins?: "none" | "current";
  readonly financialBuiltins?: boolean;
  readonly personalBuiltins?: boolean;
  /** Custom rules, appended after selected built-ins. */
  readonly rules?: readonly RuleSpec[];
}

type RuleSpec =
  | LiteralRuleSpec
  | PrefixRuleSpec
  | SuffixRuleSpec
  | PatternRuleSpec;
```

Every rule requires `id`, `severity`, and an optional `remediation` (`null`
preserves the rule default). Literal, prefix, and suffix rules add `value`.
Pattern rules add a Rust `regex`-syntax `pattern`. `captureGroup` is reserved by
the approved contract but currently rejected with
`CribraError("unsupported_capability")` if defined.

```ts
interface ScanOptions {
  /** Adds Finding.explanation. Default false. */
  readonly includeExplanations?: boolean;
}

interface ScanSource {
  readonly id: string;
  readonly source: string;
}
```

`ScanSource.id` is preserved verbatim and may be empty, non-ASCII, or
duplicated. Input order is preserved in batches.

## Result DTOs

```ts
interface Finding {
  readonly ruleId: string;
  readonly span: ByteSpan;
  readonly line: number;
  readonly column: number;
  readonly severity: Severity;
  readonly confidence: Confidence;
  readonly remediation: Remediation | null;
  readonly explanation?: Explanation;
}

interface Candidate {
  readonly kind: CandidateKind;
  readonly evidence: CandidateEvidence;
  readonly span: ByteSpan;
  readonly line: number;
  readonly column: number;
  readonly explanation: Explanation;
}

interface ScanResult {
  readonly sourceBytes: number;
  readonly findings: readonly Finding[];
  readonly candidates: readonly Candidate[];
  readonly findingCount: number;
  readonly candidateCount: number;
  readonly needsReview: boolean;
  readonly hasCritical: boolean;
}
```

`Finding` is an authoritative classification. `Candidate` is review-only
evidence; it is not promoted or transformed automatically. Both use zero-based,
half-open UTF-8 byte spans. `sourceBytes` is the UTF-8 byte length, not
`source.length`.

```ts
interface BatchEntry {
  readonly id: string;
  readonly sourceBytes: number;
  readonly result: ScanResult;
}

interface BatchResult {
  /** Same length and order as the input array. */
  readonly entries: readonly BatchEntry[];
  readonly summary: ScanSummary;
}
```

`ScanSummary` counts scanned sources and bytes, reports with and without
findings, total findings and candidates, the severity distribution, and the
`isClean`/`hasCritical` flags. `RuleMetadata` exposes `ruleId`, `kind`,
`detectionMode`, `severity`, and `remediation` for each compiled rule.

## Aliases and literal unions

```ts
type Backend = "wasm" | "native";
type Profile = "core" | "flow";
type Severity = "info" | "low" | "medium" | "high" | "critical";
type Confidence = "low" | "medium" | "high";
type DetectionMode = "matcher_only" | "deterministic" | "contextual";
type RuleKind = "literal" | "prefix" | "suffix" | "pattern";
type CandidateKind = "recovery_like_code" | "unknown";
type CandidateEvidence = "structural" | "unknown";
type ShareModeKind = "redact" | "template" | "pseudonymize" | "synthesize" | "unknown";

type Remediation =
  | "revoke_and_rotate_credential"
  | "rotate_credential"
  | "rotate_password"
  | "replace_private_key"
  | "remove_sensitive_value"
  | "review_sensitive_hash"
  | "review_password_verifier"
  | "unknown";

type Explanation =
  | { readonly kind: "classified"; readonly detectionMode: DetectionMode }
  | { readonly kind: "ambiguous"; readonly evidence: CandidateEvidence }
  | { readonly kind: "unknown" };
```

## Ownership and lifetimes

| Object | Ownership and disposal |
| --- | --- |
| `Scanner` | Owns the compiled backend engine. `dispose()` releases it and is idempotent. Scanning methods throw `disposed` afterward. |
| `StreamSession` | Owns its stream state independently of the scanner. `push()` copies the input chunk. `finish()` consumes the session and returns a detached result; `dispose()` releases it without a result. |
| `IncrementalSession` | Owns the complete mutable document. `dispose()` is idempotent; already returned snapshots remain valid. Disposing the scanner does not invalidate the session. |
| `ShareBundle` | Plain detached data. The native builder is internal, so there is no public builder or `dispose()` method. |
| Returned DTOs | Frozen plain objects and arrays. They never contain native handles, functions, or `undefined` fields. |

Stream state is `active`, `finished`, `failed`, or `disposed`. After `finish()`
or a terminal failure, `push()` and `finish()` throw
`CribraError("stream_not_active")`; after `dispose()` they throw
`CribraError("disposed")`. The stream's `disposed` getter is `true` for all
non-active states.

An incremental session exposes `disposed` for explicit disposal and `isActive`
for whether further edits may succeed. A recoverable rejection leaves the session
active; a terminal failure sets `isActive` to `false`. `snapshot()` returns the
last committed revision and report.

Transformation keys are exactly 32 bytes and are copied for the call. Core does
not retain them and makes no secure-erasure guarantee. `ShareMode` keys have the
same input-only semantics.

## UTF-8 coordinates

- `ByteSpan.start` and `ByteSpan.end` are zero-based, half-open UTF-8 byte
  offsets.
- `Finding.line` and `Finding.column` are one-based. Columns count Unicode
  scalar values, not UTF-16 code units or bytes.
- `ScanResult.sourceBytes` is the UTF-8 byte length.
- `byteSpanToTextRange(source, span)` returns a half-open UTF-16 code-unit range
  suitable for `String.prototype.slice`.

The offset helper is native-independent. It rejects malformed strings
(unpaired UTF-16 surrogates), non-integer or negative endpoints, reversed spans,
out-of-bounds spans, and spans that split a UTF-8 scalar. Incremental edits use
the same UTF-8 byte rule and accept only scalar boundaries.

## Errors

`CribraError` extends `Error` and has these stable fields:

```ts
class CribraError extends Error {
  readonly code: CribraErrorCode;
  readonly domain: ErrorDomain;
  readonly ruleId?: string;
  readonly terminal?: boolean;
}
```

Messages are fixed diagnostics and never interpolate caller source text or
matched values. `terminal` is present only for incremental failures. Use
`error instanceof CribraError` to narrow the type.

```ts
type ErrorDomain =
  | "init"
  | "scanner"
  | "capability"
  | "stream"
  | "incremental"
  | "transform"
  | "share"
  | "offset";
```

The complete public error-code union:

```ts
type CribraErrorCode =
  | "not_initialized"
  | "disposed"
  | "unsupported_capability"
  | "invalid_argument"
  | "invalid_key_length"
  | "invalid_unicode_string"
  | "invalid_span_value"
  | "invalid_span_order"
  | "span_out_of_bounds"
  | "span_not_utf8_boundary"
  | "invalid_pattern"
  | "pattern_matches_empty"
  | "missing_capture_group"
  | "empty_rule_id"
  | "duplicate_rule_id"
  | "empty_matcher"
  | "too_many_rules"
  | "automaton_build"
  | "invalid_remediation"
  | "stream_unsupported_ruleset"
  | "stream_not_active"
  | "stream_invalid_utf8"
  | "stream_incomplete_utf8"
  | "stream_candidate_limit"
  | "stream_result_limit"
  | "stream_storage_limit"
  | "stream_storage_overflow"
  | "stream_source_overflow"
  | "stream_coordinate_overflow"
  | "stream_invariant"
  | "stream_invalid_limit"
  | "stream_unknown"
  | "incremental_not_active"
  | "incremental_stale_revision"
  | "incremental_invalid_range"
  | "incremental_invalid_utf8_boundary"
  | "incremental_revision_overflow"
  | "incremental_document_length_overflow"
  | "incremental_candidate_limit"
  | "incremental_cache_limit"
  | "incremental_result_limit"
  | "incremental_transaction_storage_limit"
  | "incremental_resource_overflow"
  | "incremental_invariant"
  | "incremental_invalid_limit"
  | "incremental_offset_out_of_range"
  | "incremental_invalid_revision"
  | "incremental_unknown"
  | "transform_invalid_span"
  | "transform_invalid_utf8_boundary"
  | "transform_overlapping_spans"
  | "transform_unknown"
  | "share_missing_mode"
  | "share_source_count_mismatch"
  | "share_source_length_mismatch"
  | "share_timestamp_invalid"
  | "share_unknown"
  | "wasm_load_failed"
  | "wasm_instantiate_failed";
```

## Privacy

Reports, findings, candidates, snapshots, and error objects never retain the
scanned source or matched values. They contain rule identity, coordinates,
classification metadata, counts, and summaries. Error messages are fixed.

The caller owns the input and every transformed derivative. Transform and Share
Bundle output contains the transformed source by design. For example, output of
a redaction retains all text outside classified finding spans; review-only
candidates and undetected values may remain. Do not treat transformed output as
a proof that a document is free of sensitive data.

## Disposal matrix

| Operation | After `scanner.dispose()` | After session `dispose()` |
| --- | --- | --- |
| `scanner.scan()` / `scanBatch()` | Throws `CribraError("disposed")` | Not applicable |
| `scanner.stream()` / `incremental()` | Throws `CribraError("disposed")` | Not applicable |
| `scanner.transform()` | Throws `CribraError("disposed")` | Not applicable |
| Existing stream `push()` / `finish()` | Remains usable | Throws `CribraError("disposed")` |
| Existing incremental `applyEdit()` / `snapshot()` | Remains usable | Throws `CribraError("disposed")` |
| Previously returned results/snapshots | Remain valid | Remain valid |
| `createShareBundle(scanner, ...)` | Throws `CribraError("disposed")` | Not applicable |

The [quick start](/docs/typescript/) shows working examples for initialization,
scanning, batches, offset conversion, errors, and disposal. The [sessions
guide](/docs/typescript-sessions-and-transformations/) covers streaming,
incremental edits, transformations, and Share Bundles.
