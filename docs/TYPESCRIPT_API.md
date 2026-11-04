# Cribra TypeScript Isomorphic API Contract

Status: **partially implemented — W3.2B-2 foundation and W3.2B-3
streaming/incremental are implemented in `packages/cribra/`; transformations
and Share Bundles remain pending.**

Revision: W3.2A-1R review corrections applied.

Scope: W3.2A contract for one public TypeScript API shared by a
WebAssembly backend (initial) and a future native backend. This document
contains no implementation and requires no Core, binding, or release change.

Related contracts:

- [WebAssembly interoperability](WASM_INTEROP.md)
- [Streaming core contract](streaming.md)
- [Incremental documents](incremental.md)
- [Compatibility and distribution](COMPATIBILITY.md)
- [Ecosystem and scope](ECOSYSTEM.md)

## 1. Purpose

Cribra exposes the same Rust engine through several independently versioned
adapters. TypeScript is currently served only by the typed `wasm-bindgen`
adapter in `crates/cribra-wasm`, whose generated declarations describe
WebAssembly internals rather than a stable application API.

This document defines one isomorphic TypeScript contract that:

- hides `wasm-bindgen` classes, numeric enums, and `free()` calls;
- returns serializable data instead of native handles;
- keeps UTF-8 byte offsets explicit;
- reports real, backend-specific capabilities instead of assumed parity;
- can later be satisfied by a native backend without changing public types.

### Evidence conventions

| Label | Meaning |
| --- | --- |
| **Verified** | Observed in the current repository source and, where stated, exercised by existing runtime or Rust tests. |
| **Recommendation** | A design decision proposed by this document; not implemented. |
| **Adapter work** | Implementable in the TypeScript wrapper or through additive WASM exports, without Core changes. |
| **Future work** | Requires a Core change, a new release, or a new backend. |

Repository references use `path:line` for the audited revision on branch
`feat/typescript-api-contract`. Line numbers are evidence anchors, not API
guarantees.

## 2. Audited feature inventory

### 2.1 Rust Core (verified)

Public surface re-exported from `src/lib.rs:193-215`.

| Area | Symbols | Evidence |
| --- | --- | --- |
| Scanner construction | `Scanner::default`, `Scanner::builder`, `ScannerBuilder` | `src/scanner.rs:139`, `src/scanner_builder.rs` |
| Single and batch scan | `Scanner::scan` over `(key, source)` pairs | `src/scanner.rs:195` |
| Parallel scan | `Scanner::parallel_scan`, `parallel` feature | `src/scanner.rs:219`, `Cargo.toml:17` |
| Streaming | `Scanner::stream`, `stream_with_limits`, `StreamSession::push`, `finish` | `src/scanner.rs:124-138`, `src/stream.rs:274-300` |
| Incremental | `Scanner::incremental_session`, `incremental_session_with_limits`, `IncrementalSession::apply_edit` | `src/scanner.rs:98-122`, `src/incremental_session.rs:811` |
| Findings | `Finding`, `Location` (byte span plus line/scalar column) | `src/finding.rs`, `src/location.rs:41-83` |
| Review candidates | `SensitiveCandidate`, `SensitiveCandidateKind`, `CandidateEvidence` | `src/sensitive_candidate.rs` |
| Reports and summaries | `ScanReport`, `ScanResults`, `ScanEntry`, `ScanSummary`, `ScanQuery` | `src/scan_report.rs`, `src/scan_results.rs`, `src/scan_summary.rs` |
| Rule metadata | `RuleMetadata`, `DetectionMode`, `RuleKind`, `Scanner::rule_metadata` | `src/rule_metadata.rs`, `src/scanner.rs:286` |
| Explainability | `Finding::explanation`, `SensitiveCandidate::explanation` | `src/finding.rs:85`, `src/sensitive_candidate.rs:97` |
| Built-ins | `builtins::CURRENT` (94 rules), `builtins::financial::CURRENT` (3), `builtins::personal::CURRENT` (4) | `src/builtins/mod.rs`, `src/builtins/financial.rs:58`, `src/builtins/personal.rs:77` |
| Transformations (`transform` feature) | `redact`, `redact_with`, `template`, `template_with`, `pseudonymize`, `synthesize` | `src/transform/mod.rs:22-28` |
| Share Bundle (`transform` feature) | `ShareBundleBuilder::{build, scan_and_build, scan_and_build_with_generated_at}`, `ShareBundle`, `ShareManifest` | `src/transform/share_bundle.rs:259-366` |
| Serialization (`serde` feature) | Optional derives on findings, locations, reports, results, summaries | `Cargo.toml:18` |

### 2.2 WebAssembly adapter (verified)

The adapter is `crates/cribra-wasm` (version `0.4.7`). It projects Core
directly; it does not route through the C ABI.

| Area | Exported symbol | Evidence |
| --- | --- | --- |
| Default engine | `ScanEngine` constructor with `builtins::CURRENT` | `crates/cribra-wasm/src/lib.rs:79` |
| Rule count | `ScanEngine.rulesCount` | `crates/cribra-wasm/src/lib.rs:85` |
| Rule metadata | `ScanEngine.activeRules()` | `crates/cribra-wasm/src/metadata.rs:82` |
| Single scan | `ScanEngine.scan(string)` | `crates/cribra-wasm/src/lib.rs:90` |
| Batch scan | `ScanEngine.scanBatch(ScanBatchSource[])` | `crates/cribra-wasm/src/batch.rs:42` |
| Explanations | `explainFinding`, `candidateExplanationAt` | `crates/cribra-wasm/src/lib.rs:108`, `:854` |
| Builder | `ScanEngineBuilder` with literal/prefix/suffix/pattern and remediation variants | `crates/cribra-wasm/src/lib.rs:157-283` |
| Streaming | `stream`, `streamWithLimits`, `StreamSession.push(Uint8Array)`, `finish` | `crates/cribra-wasm/src/streaming.rs:145-218` |
| Incremental | `incremental`, `incrementalWithLimits`, `applyEdit`, `snapshot`, `isActive` | `crates/cribra-wasm/src/incremental.rs:158-256` |
| Redaction | `ScanResult.redact`, `redactWith` | `crates/cribra-wasm/src/lib.rs:773-785` |
| Templates | `ScanResult.template`, `templateWith` | `crates/cribra-wasm/src/lib.rs:789-808` |
| Pseudonymization | `ScanResult.pseudonymize` | `crates/cribra-wasm/src/lib.rs:815` |
| Synthesis | `ScanResult.synthesize` | `crates/cribra-wasm/src/lib.rs:834` |
| Share Bundle | `ShareBundleBuilder`, `scanAndBuild`, manifest and entries | `crates/cribra-wasm/src/share_bundle.rs:57-141` |
| Typed limits | `StreamLimits`, `IncrementalLimits` custom TypeScript sections | `crates/cribra-wasm/src/streaming.rs:102`, `crates/cribra-wasm/src/incremental.rs:128` |
| Adapter version | `wasmAdapterVersion()` | `crates/cribra-wasm/src/lib.rs:64` |
| Capability manifest | `capabilityManifest()`, `CapabilityManifest` getters | `crates/cribra-wasm/src/lib.rs:73`, `crates/cribra-wasm/src/capabilities.rs:41` |

Both profiles are built from the same source: `core` omits `transform`,
`flow` enables it (`crates/cribra-wasm/Cargo.toml:18-20`). The runtime
qualification harness builds both (`crates/cribra-wasm/tests/runtime/qualify.mjs`).

### 2.3 Feature matrix

Legend:

- **Rust Core** — the API exists in the authoritative crate.
- **WASM adapter (existing)** — `Exported` means callable in the current source
  adapter; `Flow only` means available only with the `transform` feature;
  `Absent` means no export exists.
- **TypeScript implementation** — `None` means specified by this document but
  not implemented in the private package. W3.2B-2 implemented the foundation and
  W3.2B-3 implemented streaming and incremental scanning. Transformations,
  Share Bundles, and final qualification remain pending.

| Capability | Rust Core | WASM adapter (existing) | TypeScript contract | TypeScript implementation |
| --- | --- | --- | --- | --- |
| Single-source scan | Yes | Exported | `Scanner.scan` | Implemented (W3.2B-2) |
| Batch scan | Yes | Exported | `Scanner.scanBatch` | Implemented (W3.2B-2) |
| Streaming | Yes | Exported | `Scanner.stream` | Implemented (W3.2B-3) |
| Incremental | Yes | Exported | `Scanner.incremental` | Implemented (W3.2B-3) |
| Built-in catalog | Yes | Exported | `ScannerOptions.builtins` | Implemented (W3.2B-2) |
| Financial/personal catalogs | Yes | Exported | `ScannerOptions.financialBuiltins`, `personalBuiltins` | Implemented (W3.2B-2) |
| Custom rules | Yes | Exported | `ScannerOptions.rules` | Implemented (W3.2B-2) |
| Rule metadata | Yes | Exported | `Scanner.rules` | Implemented (W3.2B-2) |
| Finding explanations | Yes | Exported | `Finding.explanation` (opt-in) | Implemented (W3.2B-2) |
| Candidate explanations | Yes | Exported | `Candidate.explanation` | Implemented (W3.2B-2) |
| Redaction | Yes | Flow only | `Scanner.transform` | None (pending) |
| Templates | Yes | Flow only | `Scanner.transform` | None (pending) |
| Pseudonymization | Yes | Flow only | `Scanner.transform` | None (pending) |
| Synthesis | Yes | Flow only | `Scanner.transform` | None (pending) |
| Share Bundle | Yes | Flow only | `createShareBundle` | None (pending) |
| Parallel scan | Yes (`parallel` feature) | Absent | Capability flag only | None |
| Core/adapter versions | Crate metadata only | Adapter version exported; Core version absent | `getVersion` | Implemented (adapter version only) |
| Capability probing | No | Exported | `getCapabilities` | Implemented (W3.2B-2) |
| Transform an already-stored result | Yes, with `&ScanReport` | Live native result only | Not offered | None (deferred) |
| Native backend | n/a | n/a | Same contract | None (future) |

The capability taxonomy and its separation from streaming admission are defined
in section 8.

### 2.4 Required adapter work

These items are reachable without changing Core semantics.

1. **DTO materialization.** The current `ScanResult` owns a native `ScanReport`
   (`crates/cribra-wasm/src/lib.rs:727-731`). The wrapper must iterate
   findings/candidates, copy every field into plain objects, and release the
   native result before returning. Underlying getters:
   `sourceBytes` (`:735`), `findingCount` (`:741`), `findingAt` (`:761`),
   `candidateAt` (`:841`), `candidateExplanationAt` (`:854`).
2. **Eager explanation projection.** `scan(..., { includeExplanations: true })`
   calls `explainFinding` per finding before freeing the native result.
   Core resolves explanations with a linear scan over rule metadata
   (`src/explanation.rs:56-65`), so this is `O(findings × rules)` and must stay
   opt-in.
3. **Transform materialization.** `Scanner.transform` performs one native scan,
   applies the selected transformation through `ScanResult.*`, copies the output
   string, then frees the native result and scanner-owned temporaries.
4. **Share Bundle materialization.** Build through the existing native builder,
   copy entries and manifest into data, then release the builder and bundle.
5. **Numeric and enum normalization.** Map numeric `wasm-bindgen` enums to string
   literals and validate every numeric input against JavaScript safe-integer
   limits and the target `usize` width (`crates/cribra-wasm/src/numeric.rs`).
6. **Version and capability exports.** See section 8.3.

### 2.5 Required WASM exports or Core changes

| Requirement | Why | Classification |
| --- | --- | --- |
| Core version string | `getVersion().core` cannot be derived from the WASM crate version | Adapter work, may need an additive `cribra::VERSION` constant (**Core change, needs approval**) |
| Adapter version string | `cribra-wasm` version is exposed by `wasmAdapterVersion()` | **Implemented in W3.2B-1** |
| Capability manifest | `capabilityManifest()` reflects the built Core/Flow artifact and its exports | **Implemented in W3.2B-1** |
| Batched explanation projection | Avoid `O(findings × rules)` native calls | Adapter work (optimization) |
| Report reconstruction from data | Would allow transforming a stored DTO | Future work (**Core change**) |
| Parallel batch execution in WASM | Requires threads support and qualification | Future work |

### 2.6 Semantic differences that the contract must preserve

- **Byte offsets.** `Location` exposes UTF-8 byte offsets and one-based
  line/column, where columns count Unicode scalar values
  (`src/location.rs:5-22`). They are not UTF-16 indices.
- **Rust regex syntax.** Custom pattern rules use the Rust `regex` crate
  (`Cargo.toml:25`), not ECMAScript `RegExp`. Lookarounds and backreferences
  are unavailable, and `\b`, `\d`, and Unicode classes follow Rust semantics.
- **Streaming admission.** The default built-in portfolio is not automatically
  streamable; unsupported rulesets are rejected up front
  (`docs/streaming.md`, `ROADMAP.md:127-134`). `Scanner.stream()` on the default
  scanner is expected to fail admission until a streamable ruleset is selected.
- **Incremental coverage.** Built-in and custom rules retain full incremental
  semantics (`crates/cribra-wasm/src/incremental.rs:156-158`).
- **Candidate-only sources.** Transformations affect classified findings only;
  review candidates are never rewritten (verified in
  `crates/cribra-wasm/src/lib.rs:1025-1038`).
- **Overlap policy.** Redaction merges overlapping spans; template,
  pseudonymization, and synthesis reject them (`src/transform/mod.rs:32-59`).
- **`usize` width.** All offsets and limits must fit both a JavaScript safe
  integer and the backend `usize`; on `wasm32` that is a 32-bit bound
  (`crates/cribra-wasm/src/numeric.rs:4-15`).
- **No source retention.** Reports, findings, candidates, and DTOs never retain
  source text or matched values.

## 3. Public API

### 3.1 Shape decision

**Recommendation:** explicit asynchronous `init()`, then a **synchronous**
`createScanner()` factory and plain objects with closure-held private state. No
public class, inheritance, or generated `wasm-bindgen` type appears in any
signature.

The asynchrony is confined to module initialization. `wasm-bindgen` module
instantiation is asynchronous, but once `await init()` resolves, the generated
`ScanEngine` constructor and `ScanEngineBuilder.build()` are synchronous
(`crates/cribra-wasm/src/lib.rs:79`, `crates/cribra-wasm/src/lib.rs:280`).
Scanner construction therefore has no hidden asynchronous work. Only `init()`
returns a promise; every other public entry point is synchronous.

Alternatives considered:

| Shape | Advantage | Why not selected |
| --- | --- | --- |
| Module-level `scan()` functions | Smallest call site | Would recompile 94+ rules per call or hide a process-global scanner |
| `new Scanner()` class | Familiar to WASM users | Leaks construction/lifecycle shape, complicates the native seam |
| Synchronous factory after explicit `init()` | One compilation per scanner, no class surface, no hidden async | Selected |

`Scanner`, `StreamSession`, and `IncrementalSession` own native state and expose
`dispose()`. `ScanResult`, `Candidate`, `Finding`, `ScanSummary`, and
`ShareBundle` are autonomous DTOs with no native handle and no `dispose()`.

### 3.2 Entry point and initialization

```ts
export type Backend = "wasm" | "native";
export type Profile = "core" | "flow";

export interface InitOptions {
  /** Explicit WASM bytes or URL. Omitted: resolve relative to the package asset. */
  readonly wasm?: Uint8Array | ArrayBuffer | URL | string;
}

/**
 * Initializes the backend once per module instance.
 * Idempotent and concurrency-safe: concurrent calls share one promise.
 * A failed initialization leaves no partial state and may be retried.
 */
export function init(options?: InitOptions): Promise<void>;

export function isInitialized(): boolean;

/** Throws CribraError("not_initialized") before init(). */
export function getCapabilities(): Capabilities;

/** Throws CribraError("not_initialized") before init(). */
export function getVersion(): VersionInfo;
```

### 3.3 Scanner

```ts
export interface ScannerOptions {
  /** Defaults to "current". "none" selects an empty built-in catalog. */
  readonly builtins?: "none" | "current";
  readonly financialBuiltins?: boolean;
  readonly personalBuiltins?: boolean;
  /** Custom rules, appended after selected built-ins. */
  readonly rules?: readonly RuleSpec[];
}

export type RuleSpec =
  | LiteralRuleSpec
  | PrefixRuleSpec
  | SuffixRuleSpec
  | PatternRuleSpec;

interface RuleSpecBase {
  readonly id: string;
  readonly severity: Severity;
  /** Omitted or null preserves the rule's default remediation. */
  readonly remediation?: Remediation | null;
}

export interface LiteralRuleSpec extends RuleSpecBase {
  readonly kind: "literal";
  readonly value: string;
}

export interface PrefixRuleSpec extends RuleSpecBase {
  readonly kind: "prefix";
  readonly value: string;
}

export interface SuffixRuleSpec extends RuleSpecBase {
  readonly kind: "suffix";
  readonly value: string;
}

export interface PatternRuleSpec extends RuleSpecBase {
  readonly kind: "pattern";
  /** Rust `regex` syntax. */
  readonly pattern: string;
  readonly captureGroup?: number;
}

export interface Scanner {
  readonly rulesCount: number;
  readonly rules: readonly RuleMetadata[];
  readonly disposed: boolean;

  scan(source: string, options?: ScanOptions): ScanResult;
  scanBatch(sources: readonly ScanSource[], options?: ScanOptions): BatchResult;
  transform(source: string, request: TransformRequest): TransformResult;

  stream(options?: StreamOptions): StreamSession;
  incremental(source: string, options?: IncrementalOptions): IncrementalSession;

  /** Idempotent. After disposal, scanning methods throw CribraError("disposed"). */
  dispose(): void;
}

/**
 * Requires a successfully initialized backend.
 * Throws CribraError("not_initialized") otherwise.
 * Synchronous: rule validation and compilation complete before it returns.
 */
export function createScanner(options?: ScannerOptions): Scanner;
```

All methods are declared on every backend, including Core-profile builds. An
operation that the initialized backend cannot perform throws
`CribraError("unsupported_capability")`; methods never disappear at runtime.
Capability flags describe availability, and callers may branch on them before
calling an unsupported operation.

`createScanner` is synchronous and does not call `init()` implicitly. Calling
`init()` first is required and surfaces load failures before any scanner exists.
This keeps one `Scanner` interface across Core and Flow and avoids a
`Promise<Scanner>` contract that the WebAssembly backend does not require.

### 3.4 Scan requests and results

```ts
export interface ScanOptions {
  /** Adds Finding.explanation. Default false because resolution is O(findings × rules). */
  readonly includeExplanations?: boolean;
}

export interface ScanSource {
  /** Preserved verbatim; may be empty, non-ASCII, or duplicated. */
  readonly id: string;
  readonly source: string;
}

export interface ScanResult {
  /** UTF-8 byte length of the scanned source, not a JavaScript string length. */
  readonly sourceBytes: number;

  readonly findings: readonly Finding[];
  readonly candidates: readonly Candidate[];

  readonly findingCount: number;
  readonly candidateCount: number;
  readonly needsReview: boolean;
  readonly hasCritical: boolean;
}

export interface Finding {
  readonly ruleId: string;
  readonly span: ByteSpan;
  /** One-based; columns count Unicode scalar values. */
  readonly line: number;
  readonly column: number;
  readonly severity: Severity;
  readonly confidence: Confidence;
  /** null means Core assigned no remediation. */
  readonly remediation: Remediation | null;
  /** Present only when ScanOptions.includeExplanations is true. */
  readonly explanation?: Explanation;
}

export interface Candidate {
  readonly kind: CandidateKind;
  readonly evidence: CandidateEvidence;
  readonly span: ByteSpan;
  readonly line: number;
  readonly column: number;
  readonly explanation: Explanation;
}

export interface BatchEntry {
  readonly id: string;
  readonly sourceBytes: number;
  readonly result: ScanResult;
}

export interface BatchResult {
  /** Same length and order as the input array. */
  readonly entries: readonly BatchEntry[];
  readonly summary: ScanSummary;
}

export interface ScanSummary {
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
}
```

### 3.5 Transformations

A transformation needs both the source and Core's scan report. The current WASM
adapter can only transform against a live native result
(`crates/cribra-wasm/src/lib.rs:773-838`). Because `ScanResult` is a
detached DTO, transforms are **scan-coupled**: one call scans and transforms
atomically, then releases native state.

```ts
export interface RedactRequest {
  readonly kind: "redact";
  /** Omitted uses Core's "[REDACTED]". */
  readonly replacement?: string;
}

export interface TemplateRequest {
  readonly kind: "template";
  /** Omitted uses Core's "CRIBRA". */
  readonly namespace?: string;
  readonly numbered?: boolean;
}

export interface PseudonymizeRequest {
  readonly kind: "pseudonymize";
  /**
   * Exactly 32 bytes. Copied into backend memory for the call; Core does not
   * retain it. JavaScript cannot promise secure erasure of either the caller's
   * buffer or backend copies; zeroing the caller's buffer afterwards is
   * best-effort hygiene, not a security guarantee.
   */
  readonly key: Uint8Array;
  /** Omitted uses "cribra_pseudo_". */
  readonly prefix?: string;
  /** Clamped by Core to 8..=32. Omitted uses 16. */
  readonly digestBytes?: number;
}

export interface SynthesizeRequest {
  readonly kind: "synthesize";
  /**
   * Exactly 32 bytes. Copied into backend memory for the call; Core does not
   * retain it. JavaScript cannot promise secure erasure of either the caller's
   * buffer or backend copies; zeroing the caller's buffer afterwards is
   * best-effort hygiene, not a security guarantee.
   */
  readonly key: Uint8Array;
  /** Omitted uses "cribra_synthetic". */
  readonly marker?: string;
}

export type TransformRequest =
  | RedactRequest
  | TemplateRequest
  | PseudonymizeRequest
  | SynthesizeRequest;

export interface TransformResult {
  /** Findings for the same scan, so callers do not scan twice. */
  readonly scan: ScanResult;
  /** Transformed source. Candidates are never rewritten. */
  readonly output: string;
}
```

Consequences:

- `transform()` is present on every `Scanner`. On a Core-profile backend it
  throws `CribraError("unsupported_capability")`; it is never removed.
- Transforming a `ScanResult` obtained earlier is **deferred beyond v1**.
  Callers that need both analysis and output must use `transform()`, which
  returns both from one scan.
- Multiple transformations of one source are **deferred beyond v1**. Until a
  future report-reconstruction or multi-output export exists, each
  transformation requires its own scan-coupled call.
- Keys are input-only. Core copies them for the call and does not retain them.
  The contract makes no claim of secure memory erasure: JavaScript cannot
  guarantee zeroization of `Uint8Array` storage, and backend copies cannot be
  verified as erased.

### 3.6 Streaming

```ts
export interface StreamOptions {
  readonly limits?: StreamLimits;
}

export interface StreamLimits {
  readonly maxCandidates?: number;
  readonly maxStreamStorageBytes?: number;
  readonly maxRetainedResults?: number;
}

export interface StreamSession {
  readonly disposed: boolean;

  /** Bytes in source order. UTF-8 scalars and CRLF may cross chunks. */
  push(chunk: Uint8Array): void;

  /** Consumes the session and returns the complete report. */
  finish(): ScanResult;

  /** Releases the session without a report. Idempotent. */
  dispose(): void;
}
```

The session owns its Core state and does not borrow the creating `Scanner`
(`crates/cribra-wasm/src/streaming.rs:143-144`). Disposing the scanner leaves
the session usable, and disposing the session leaves the scanner usable.

### 3.7 Incremental

```ts
export interface IncrementalOptions {
  readonly limits?: IncrementalLimits;
}

export interface IncrementalLimits {
  readonly maxCandidates?: number;
  readonly maxRetainedCacheRecords?: number;
  readonly maxMaterializedResults?: number;
  readonly maxTransactionStorageBytes?: number;
}

export interface IncrementalSession {
  readonly disposed: boolean;
  readonly isActive: boolean;

  /**
   * Replaces the half-open UTF-8 byte range [start, end).
   * expectedRevision must equal the last committed revision.
   * Returns the next revision. Insertion is start === end; deletion is "".
   */
  applyEdit(
    expectedRevision: bigint,
    start: number,
    end: number,
    replacement: string,
  ): bigint;

  /** Clones the committed revision and its complete report into a DTO. */
  snapshot(): IncrementalSnapshot;

  /** Idempotent. Does not invalidate snapshots already returned. */
  dispose(): void;
}

export interface IncrementalSnapshot {
  readonly revision: bigint;
  readonly result: ScanResult;
}
```

`bigint` is retained because Core revisions are `u64` and the adapter rejects
lossy conversion (`crates/cribra-wasm/src/incremental.rs:231-232`). Using
`number` would silently cap the revision space.

The session owns its Core document and compiled-rule state
(`crates/cribra-wasm/src/incremental.rs:207-209`). Disposing the creating
scanner leaves the session and its snapshots usable; disposing the session
leaves the scanner usable.

### 3.8 Share Bundles

```ts
export interface ShareBundleOptions {
  readonly mode: ShareMode;
  /** Defaults to the current time obtained by the backend. */
  readonly generatedAt?: Date;
}

export type ShareMode =
  | { readonly kind: "redact" }
  | { readonly kind: "template" }
  | {
      readonly kind: "pseudonymize";
      /** Exactly 32 bytes; input-only, with no guaranteed secure erasure. */
      readonly key: Uint8Array;
      readonly prefix?: string;
      readonly digestBytes?: number;
    }
  | {
      readonly kind: "synthesize";
      /** Exactly 32 bytes; input-only, with no guaranteed secure erasure. */
      readonly key: Uint8Array;
      readonly marker?: string;
    };

export interface ShareBundleEntry {
  readonly id: string;
  readonly content: string;
}

export interface ShareManifest {
  readonly mode: ShareModeKind;
  readonly generatedAtSeconds: number;
  readonly generatedAtNanoseconds: number;
  readonly summary: ScanSummary;
}

export interface ShareBundle {
  readonly entries: readonly ShareBundleEntry[];
  readonly manifest: ShareManifest;
  readonly summary: ScanSummary;
}

/**
 * Scans and transforms the whole ordered collection atomically.
 * Requires the Flow profile; capability `shareBundle === false` otherwise.
 */
export function createShareBundle(
  scanner: Scanner,
  sources: readonly ScanSource[],
  options: ShareBundleOptions,
): ShareBundle;
```

The native `ShareBundleBuilder` is an implementation detail allocated and
released inside this call. No builder object is public, so no `dispose()` is
required for Share Bundle data.

`createShareBundle` is always declared. On a Core-profile backend it throws
`CribraError("unsupported_capability")`. It does not retain the scanner after
the call returns.

### 3.9 Data enums

```ts
export type Severity = "info" | "low" | "medium" | "high" | "critical";
export type Confidence = "low" | "medium" | "high";
export type DetectionMode = "matcher_only" | "deterministic" | "contextual";
export type RuleKind = "literal" | "prefix" | "suffix" | "pattern";
export type CandidateKind = "recovery_like_code" | "unknown";
export type CandidateEvidence = "structural" | "unknown";

export type Remediation =
  | "revoke_and_rotate_credential"
  | "rotate_credential"
  | "rotate_password"
  | "replace_private_key"
  | "remove_sensitive_value"
  | "review_sensitive_hash"
  | "review_password_verifier"
  | "unknown";

export type ShareModeKind =
  | "redact"
  | "template"
  | "pseudonymize"
  | "synthesize"
  | "unknown";

export type Explanation =
  | { readonly kind: "classified"; readonly detectionMode: DetectionMode }
  | { readonly kind: "ambiguous"; readonly evidence: CandidateEvidence }
  | { readonly kind: "unknown" };

export interface RuleMetadata {
  readonly ruleId: string;
  readonly kind: RuleKind;
  readonly detectionMode: DetectionMode;
  readonly severity: Severity;
  readonly remediation: Remediation | null;
}
```

String values match Core's serde `snake_case` representation where one exists
(`src/severity.rs`, `src/remediation.rs`, `src/rule_metadata.rs`,
`src/sensitive_candidate.rs`). `"unknown"` is an explicit forward-compatibility
member, not a free-form string: it is returned only when a future backend emits
a variant this wrapper does not understand.

## 4. Initialization and lifecycle

### 4.1 Explicit, side-effect-free initialization

**Recommendation:** the package performs no top-level `await`, no WASM fetch, and
no instantiation at import time.

Loading requirements:

- No static Node built-in import may appear in a module reachable by a browser
  build. Node support uses package `exports` conditions or a dynamic
  `import("node:fs/promises")` evaluated only after runtime environment
  detection.
- Browser: `init()` uses `WebAssembly.instantiateStreaming` when the asset is a
  URL and falls back to `arrayBuffer()` when the MIME type is unavailable.
- Node.js: `init()` reads the packaged asset through the Node-only loader, or
  uses caller-supplied bytes.
- Default asset resolution uses `new URL("./wasm/cribra_bg.wasm",
  import.meta.url)` only in the browser/default entry, never in a CommonJS or
  Node-specific module compiled into a browser bundle.
- Bundler compatibility is **not assumed**. Vite, webpack 5, Rollup, and esbuild
  behavior is a W3.2C qualification item. Until each is tested, the contract
  guarantees only explicit `InitOptions.wasm` bytes or URLs and the Node ESM
  path.
- React Native is **not** claimed. It is a separate qualification target.

### 4.2 Repeated and concurrent initialization

- `init()` stores one in-flight promise. Concurrent callers await the same
  promise.
- A resolved initialization is terminal for the module instance; later calls
  return immediately.
- A rejected initialization clears the stored promise so callers may retry.
  No partially initialized state is retained.
- After initialization resolves, `createScanner`, `scan`, transformations,
  sessions, `getCapabilities`, and `getVersion` are synchronous. Before
  initialization, `createScanner`, `getCapabilities`, `getVersion`, and every
  scanning entry point throw `CribraError("not_initialized")`; they never await
  or auto-initialize.

### 4.3 Resource ownership

| Type | Native handle | `dispose()` | Notes |
| --- | --- | --- | --- |
| `Scanner` | Yes | Required, idempotent | Owns the compiled `ScanEngine`; does not own sessions |
| `StreamSession` | Yes | Required, idempotent | Independent handle; `finish()` also disposes |
| `IncrementalSession` | Yes | Required, idempotent | Independent handle; `applyEdit` after disposal throws `disposed` |
| `ScanResult` | No | Not provided | Plain DTO; valid after the scanner is disposed |
| `Finding`, `Candidate` | No | Not provided | Plain data |
| `BatchResult` | No | Not provided | Plain data |
| `IncrementalSnapshot` | No | Not provided | Plain data; valid after session disposal |
| `ShareBundle` | No | Not provided | Plain data |
| `CribraError` | No | Not provided | Standard `Error` subclass |

Rules:

1. Disposal is idempotent. A second call is a no-op.
2. Any operation that would need the released handle throws
   `CribraError("disposed")`.
3. Results never borrow from their producer. Releasing a scanner never
   invalidates a result it returned.
4. **Sessions are independent of their parent scanner.** A `StreamSession` or
   `IncrementalSession` owns its Core state and remains usable after the scanner
   is disposed. Disposing a session never disposes the scanner or any sibling
   session. A session does not keep the `Scanner` object alive; where Core
   shares immutable compiled rules, it does so through its own internal
   reference, not through the public handle.
5. `finish()` consumes only its own stream session. `snapshot()` returns data
   that remains valid after the session is disposed.
6. `createShareBundle` borrows the scanner only for the duration of the call.
   The returned `ShareBundle` is independent of both the scanner and the
   internal native builder.
7. `FinalizationRegistry` may be used as a best-effort safety net for forgotten
   handles. It is never part of the correctness contract, because collection
   timing is unspecified.

## 5. Data and offset contract

### 5.1 Offsets

- `ByteSpan` and `Finding.span` are half-open UTF-8 byte ranges `[start, end)`,
  exactly as `Location` defines them (`src/location.rs:5-22`).
- `line` is one-based. `column` is one-based and counts Unicode scalar values,
  not UTF-16 code units and not grapheme clusters.
- `ScanResult.sourceBytes` is the UTF-8 byte length.
- No API silently converts a byte offset into a JavaScript string index.

```ts
export interface ByteSpan {
  readonly start: number;
  readonly end: number;
}

/** Half-open UTF-16 code-unit range, suitable for String.prototype.slice. */
export interface TextRange {
  readonly start: number;
  readonly end: number;
}

/**
 * Converts a UTF-8 byte span into UTF-16 code-unit indices.
 * Throws CribraError; never clamps or rounds.
 */
export function byteSpanToTextRange(source: string, span: ByteSpan): TextRange;
```

`byteSpanToTextRange` validates in this order:

| Condition | Error code |
| --- | --- |
| `source` contains an unpaired surrogate | `invalid_unicode_string` |
| `start` or `end` is not a nonnegative safe integer | `invalid_span_value` |
| `start > end` | `invalid_span_order` |
| `end > utf8ByteLength(source)` | `span_out_of_bounds` |
| `start` or `end` is not a UTF-8 scalar boundary | `span_not_utf8_boundary` |

Returned `start`/`end` are half-open UTF-16 indices. The helper is pure and has
no native dependency, so it remains usable after `dispose()`.

Encoding rules:

- The contract is defined for well-formed Unicode strings.
- **Malformed JavaScript strings:** if `source` contains an unpaired high or low
  surrogate, `byteSpanToTextRange` throws `invalid_unicode_string` before
  computing any offset. It never substitutes U+FFFD, clamps, or returns a
  best-effort index.
- Scan input follows the platform boundary, not this helper. `wasm-bindgen`
  encodes JavaScript strings as UTF-8 and replaces unpaired surrogates with
  U+FFFD. A caller that passes a malformed string therefore receives offsets
  into the replaced encoding, not into the original JavaScript string. Such
  input is outside the offset contract; callers that need addressable offsets
  must pass well-formed strings.
- The helper must not use `TextEncoder` on substrings, because that replaces
  unpaired surrogates and can desynchronize byte lengths. It walks code points,
  rejects unpaired surrogates, and accumulates UTF-8 lengths explicitly.

### 5.2 Serializable data

Every type in section 3.4–3.8 contains only strings, numbers, booleans,
`bigint`, arrays, plain objects, and `null`. No `Map`, `Date` (except the
`ShareBundleOptions.generatedAt` input), typed array (except transformation
keys), or class instance appears in a returned DTO. `JSON.stringify` therefore
works on any result, with the documented `bigint` caveat for revisions.

## 6. Error model

```ts
export type ErrorDomain =
  | "init"
  | "scanner"
  | "capability"
  | "stream"
  | "incremental"
  | "transform"
  | "share"
  | "offset";

export type CribraErrorCode =
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
  | "share_missing_mode"
  | "share_source_count_mismatch"
  | "share_source_length_mismatch"
  | "share_timestamp_invalid"
  | "wasm_load_failed"
  | "wasm_instantiate_failed";

export class CribraError extends Error {
  readonly name: "CribraError";
  readonly code: CribraErrorCode;
  readonly domain: ErrorDomain;
  /** Present for incremental failures; true means the session is terminal. */
  readonly terminal?: boolean;
  /** Present for scanner-construction failures that name a rule. */
  readonly ruleId?: string;
  /**
   * Optional, non-serializable backend detail. Never guaranteed, never part of
   * the stable contract, and never contains source text, matched values, or keys.
   */
  readonly cause?: unknown;
}
```

Mapping rules:

- **Verified source sets:** `ScannerBuildErrorCode`
  (`crates/cribra-wasm/src/build_error.rs:7-26`), `StreamErrorCode`
  (`crates/cribra-wasm/src/streaming.rs:10-37`), `IncrementalErrorCode`
  (`crates/cribra-wasm/src/incremental.rs:9-42`), and `TransformError`
  (`src/transform/mod.rs:32-78`). Every variant has a stable string code.
- **One code per failure.** Normalization uses a static mapping table, never
  substring matching on diagnostic text. An unmapped backend variant becomes
  its domain's `*_unknown` code with `cause` attached when safe.
- **No raw backend errors escape.** A backend `JsError` or generated error
  object is converted before crossing the public boundary; callers never receive
  a `wasm-bindgen` error instance.
- **`unsupported_capability`** is produced by the wrapper when a declared method
  exists but the initialized backend cannot perform it (for example
  `transform()` on a Core-profile build). It is distinct from
  `stream_unsupported_ruleset`, which is a per-ruleset streaming admission
  failure on a backend that does support streaming.
- Numeric `wasm-bindgen` enum values are mapped, not re-exported.
- `unknown`/`*_unknown` codes are forward-compatibility sinks, not expected
  outcomes in a matched package/adapter pair.

### 6.1 Error message and cause contract

- `code` and `domain` are the machine contract. `message` is a diagnostic and
  may change between patch releases.
- Wrapper-generated messages are fixed per code. They never interpolate source
  text, matched values, candidate values, rule matcher payloads, or key bytes.
  Stable metadata such as `ruleId`, byte lengths, and counters may appear.
- Backend diagnostics are source-free by adapter contract
  (`crates/cribra-wasm/src/streaming.rs:39-47`), but they are still treated as
  diagnostics. The wrapper may replace them with the fixed message for the
  normalized code.
- `cause` is optional and may be omitted when a backend detail cannot be proven
  source-free. It may be a native object, is not JSON-serializable, and must not
  be inspected or parsed by callers. It must be defined as non-enumerable so it
  is excluded from `JSON.stringify`.
- `stack` is host-dependent and not part of the contract.

### 6.2 Sensitive-data protection

Errors must not become a data-exfiltration channel:

- Never include scanned source, source excerpts, matched or candidate values,
  normalization payloads, transformation output, or key material.
- `ruleId`, `code`, `domain`, `line`, `column`, byte lengths, and aggregate
  counters are safe metadata.
- Error objects are not redaction targets and are not scanned. A caller that
  adds source text to an error message owns that disclosure.
- Recommended logging is limited to `name`, `code`, `domain`, and `ruleId`.
  Logging `cause` or `stack` should be an explicit opt-in.
- No API claims secure erasure of keys or error payloads; see section 3.5.

Errors never contain source text or matched sensitive values.

## 7. Streaming versus incremental

These are distinct capabilities and must not share a lifecycle.

| Property | Streaming (`StreamSession`) | Incremental (`IncrementalSession`) |
| --- | --- | --- |
| Input | Ordered byte chunks | One owned UTF-8 document plus replacement edits |
| Mutation | Append-only | Random-access `[start, end)` replacement |
| Result timing | Only at `finish()` | After every successful edit, via `snapshot()` |
| Revision protocol | None | `u64` revision, checked `expectedRevision` |
| Source access | None | `sourceBytes` only; no document accessor |
| Terminal failure | Any push failure discards the session | Resource/invariant failure blocks edits; snapshots remain readable |
| Ruleset coverage | Admitted rulesets only | Full Core semantics |
| Limits | candidates, stream storage, retained results | candidates, cache records, materialized results, transaction storage |

### 7.1 Streaming semantics (verified)

Source: `src/stream.rs`, `crates/cribra-wasm/src/streaming.rs`,
`docs/streaming.md`.

- `push` accepts fragments in source order. UTF-8 scalars and CRLF may cross
  fragment boundaries. Empty pushes while active are no-ops.
- No caller buffer is retained; the caller may reuse it immediately.
- There is no pre-EOF result API.
- `finish()` validates genuine EOF, completes pending work, and returns one
  complete `ScanReport`. Malformed UTF-8 or an incomplete scalar at EOF fails
  without partial output.
- Unsupported rulesets fail admission when the session is opened. There is no
  whole-source fallback and no silent semantic degradation.
- A push failure is terminal: the session and partial output are discarded.
- Limits bound logical payload, not total heap. They exclude allocator
  overhead, compiled automata, validator workspace, and output records, which
  have their own limit.
- Ordering and overlap semantics match whole-source scanning for admitted
  configurations.

### 7.2 Incremental semantics (verified)

Source: `src/incremental_session.rs`, `crates/cribra-wasm/src/incremental.rs`,
`docs/incremental.md`.

- The session owns the UTF-8 document. The public API never exposes its text,
  only `sourceBytes`.
- `applyEdit` replaces the half-open UTF-8 byte range `[start, end)`.
  Insertion is `start === end`; deletion uses an empty replacement.
- `expectedRevision` must equal the last committed revision, otherwise the call
  fails with `incremental_stale_revision` and the document is unchanged.
- Every successful edit, including a no-op, publishes the next exact revision
  and a complete report equivalent to a fresh whole-source scan.
- Recoverable input errors (stale revision, invalid range, non-boundary range)
  leave the session active.
- Resource and invariant failures make the session terminal; the last committed
  source and report remain readable through `snapshot()`.
- Revision and document-length overflow are checked, never wrapped.
- `snapshot()` clones metadata only; it retains no source and outlives the
  session.
- Retained-cache and materialized-result limits are logical budgets, not total
  heap caps.

### 7.3 Disposal and ordering guarantees

- `finish()` consumes the stream; a later `push` or `finish` throws
  `stream_not_active`.
- `dispose()` on `StreamSession` discards partial state without a report.
- `dispose()` on `IncrementalSession` releases the document; returned snapshots
  remain valid.
- **Parent/child independence:** disposing a `Scanner` does not dispose any
  `StreamSession` or `IncrementalSession` it created. Those sessions own their
  Core state (`crates/cribra-wasm/src/streaming.rs:143-144`,
  `crates/cribra-wasm/src/incremental.rs:207-209`) and remain usable until
  disposed individually.
- Disposing a session does not dispose the scanner, its other sessions, or
  results already returned.
- Findings are returned in deterministic source order. Batch entries preserve
  input order exactly. Duplicate or empty IDs are preserved, never normalized.

## 8. Capabilities and version metadata

### 8.1 Typed capabilities

```ts
export interface Capabilities {
  readonly backend: Backend;
  readonly profile: Profile;

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
```

Rules:

1. Values describe the **initialized backend and built artifact**. They are not
   compile-time constants and not promises about future backends.
2. `getCapabilities()` returns a frozen snapshot. A future backend may return a
   different combination.
3. A capability is `true` only when the corresponding operation is actually
   implemented and qualified on that backend. Core parity alone is insufficient.
4. Consumers should branch on the capability, not on `backend` or `profile`.
5. **Capabilities are backend-level only.** They do not describe whether a
   particular scanner's rule set is admissible for streaming. Streaming
   admission is a per-scanner, per-ruleset property and is reported by
   `stream()` through `stream_unsupported_ruleset`. `streaming: true` never
   implies that any given scanner can stream.
6. Unsupported backend-level operations keep their declared method and throw
   `unsupported_capability`; capability flags let callers avoid the call, but
   method presence never varies at runtime.

### 8.2 Verified values for the current WASM builds

| Capability | Core profile | Flow profile | Basis |
| --- | --- | --- | --- |
| `singleSourceScan` | `true` | `true` | `ScanEngine.scan` |
| `batchScan` | `true` | `true` | `ScanEngine.scanBatch` |
| `streaming` | `true` | `true` | Operation exported; admission is per ruleset |
| `incremental` | `true` | `true` | Full document session |
| `redaction` | `false` | `true` | `transform` feature |
| `templating` | `false` | `true` | `transform` feature |
| `pseudonymization` | `false` | `true` | `transform` feature |
| `synthesis` | `false` | `true` | `transform` feature |
| `shareBundle` | `false` | `true` | `transform` feature |
| `ruleMetadata` | `true` | `true` | `activeRules()` |
| `explanations` | `true` | `true` | `explainFinding` |
| `parallel` | `false` | `false` | No WASM export |
| `utf16OffsetConversion` | `true` | `true` | Wrapper helper |

`streaming: true` means the backend exports the operation, not that the current
scanner is admitted. `stream()` must still be allowed to reject a ruleset with
`stream_unsupported_ruleset`, and that failure leaves the scanner usable.

### 8.3 WASM metadata exports (W3.2B-1)

`wasmAdapterVersion()` and `capabilityManifest()` are implemented additively
by `crates/cribra-wasm` and qualified for both Core and Flow. Still unavailable:

```text
coreVersion(): string | undefined
```

The implemented exports are:

```text
wasmAdapterVersion(): string
capabilityManifest(): CapabilityManifest
```

`wasmAdapterVersion()` returns the adapter's compile-time `CARGO_PKG_VERSION`.
It is never substituted for Core's version.

`CapabilityManifest` is a typed record of booleans and enums, not free-form
strings. It is a generated `wasm-bindgen` class: the wrapper copies the getters
into a plain object and calls `free()` in a `try`/`finally` block. The adapter
types `CapabilityManifest`, `BackendKind` and `ProfileKind` are internal and
must not appear in the public TypeScript package surface; the wrapper maps them
to its plain DTO, string literals, and its own `utf16OffsetConversion: true`.
The wrapper validates the copied record at initialization and fails closed if
it is missing or malformed.

`coreVersion` may require an additive `pub const VERSION: &str` in the `cribra`
crate. That is a **Core change** and must be approved separately; until then
`getVersion().core` is absent and no export exists for it.

### 8.4 Versions

```ts
export interface VersionInfo {
  /** Version of the TypeScript package itself. Always known. */
  readonly package: string;
  /** Cribra Core version, only when the backend can prove it. */
  readonly core?: string;
  /** WebAssembly adapter version, only when the backend can prove it. */
  readonly wasmAdapter?: string;
}
```

No field is ever synthesized. Absence is represented by omitting the property,
never by `"unknown"`, `"0.0.0"`, or an inferred value.

## 9. Packaging

### 9.1 Layout (recommended)

```text
packages/cribra/                 # npm identity pending (section 12)
├── package.json
├── tsconfig.json
├── src/
│   ├── index.ts                 # public entry; re-exports only
│   ├── init.ts                  # load/instantiate, idempotent lifecycle
│   ├── scanner.ts               # createScanner and Scanner
│   ├── result.ts                # DTO materialization
│   ├── transform.ts             # scan-coupled transforms
│   ├── stream.ts
│   ├── incremental.ts
│   ├── share.ts
│   ├── offset.ts                # byteSpanToTextRange
│   ├── errors.ts
│   └── types.ts
├── generated/cribra/            # wasm-bindgen output; never exported
└── dist/
    ├── index.js
    ├── index.d.ts
    └── wasm/
        ├── cribra.js
        ├── cribra.d.ts          # internal
        └── cribra_bg.wasm
```

The generated `wasm-bindgen` directory is an implementation artifact. Its
classes and numeric enums must not appear in `dist/index.d.ts` or in any
exported signature.

### 9.2 `package.json` strategy

```json
{
  "name": "@cribra/cribra",
  "type": "module",
  "sideEffects": false,
  "exports": {
    ".": {
      "types": "./dist/index.d.ts",
      "browser": "./dist/index.browser.js",
      "node": "./dist/index.node.js",
      "default": "./dist/index.js"
    },
    "./package.json": "./package.json"
  },
  "files": ["dist", "README.md", "LICENSE"],
  "engines": { "node": ">=20" }
}
```

- **ESM-first.** One ESM build and one declaration build. CommonJS is out of v1
  scope (section 12); dual publishing would introduce the dual-package hazard
  for stateful WASM initialization.
- **Environment-specific entries.** The `browser` and `default` entries never
  reference Node built-ins. Only `index.node.js` contains the Node asset loader.
- **No runtime dependencies.** `wasm-bindgen` output is vendored under `dist/`.
- **Asset included in the tarball.** Shipping `.wasm` inside `files` avoids a
  second download, a CDN dependency, and version skew. Callers may still pass
  explicit bytes or a URL to `init()`.
- **Subpath exports.** `./wasm` is intentionally *not* exported: it would leak
  generated internals. If advanced consumers need direct access, that requires
  a deliberate decision, not a default.
- **Publishing.** `publishConfig.access`, repository, license, and provenance
  are required. No publish happens in this workstream.
- **Runtime baseline.** Node 20+ is the working assumption; the final baseline
  and browser matrix are verified in W3.2C, not assumed here.

### 9.3 WASM asset loading

`init()` resolves the asset in this order:

1. `InitOptions.wasm` when provided (bytes, `ArrayBuffer`, `URL`, or string URL).
2. The environment entry selected by the `exports` map:
   - `index.node.js` uses a dynamic `import("node:fs/promises")` inside its
     Node-only loader.
   - `index.browser.js` and `index.js` use
     `new URL("./wasm/cribra_bg.wasm", import.meta.url)` and pass the result to
     `fetch` or the host bundler.
3. If neither an explicit asset nor a resolvable package asset exists, `init()`
   rejects with `wasm_load_failed`. There is no CDN or network fallback beyond
   the resolved asset URL.

No browser-reachable module imports `node:*`, statically or through a top-level
dynamic import. Bundler rewriting of `new URL(..., import.meta.url)` is
**not assumed** to work uniformly: Vite, webpack 5, Rollup, and esbuild are
qualified separately in W3.2C. Until then, callers can always pass explicit
bytes to `init()`.

### 9.4 Future native backend

- The public `index.ts` types stay identical.
- `init({ backend: "native" })` or a separate `@cribra/cribra/native` entry
  selects an implementation that maps the same DTOs.
- Backend-specific capability fields may differ; no field may be added to the
  common contract without a versioned minor release.
- No native backend exists today.

## 10. Compatibility and limitations

### 10.1 Confirmed limitations

| Limitation | Impact | Status |
| --- | --- | --- |
| TypeScript API partially implemented | Foundation, scanning, streaming, and incremental sessions exist; transforms and Share Bundles do not | W3.2B-2/W3.2B-3 |
| No parallel execution in WASM | `capabilities.parallel === false`; `scanBatch` stays sequential | Deferred |
| Default built-ins are not streamable | `stream()` rejects the default scanner with `stream_unsupported_ruleset` | Core/streaming contract |
| Transforms require Flow | Methods remain declared; Core profile throws `unsupported_capability` | Verified feature gate; wrapper behavior specified |
| No report reconstruction | A stored `ScanResult` cannot be transformed later | Deferred beyond v1 |
| No multi-transform call | One transform request per scan | Deferred beyond v1 |
| No Core version export | `getVersion().core` stays absent; `wasmAdapterVersion()` is exported and must not be used in its place | Adapter/Core work |
| Rust regex syntax | Custom patterns differ from JavaScript `RegExp` | Verified semantic difference |
| `usize` width | Offsets/limits must fit 32-bit `usize` on `wasm32` | Verified |
| Published WASM artifact lag | `cribra-wasm 0.4.7` predates batch and Share Bundle work | Release dependency |
| No CJS build | CommonJS consumers cannot `require()` the package | Out of v1 scope |
| No React Native qualification | Not supported or claimed | Out of scope |

The published-artifact lag is recorded in `docs/ECOSYSTEM.md:95-99` and
`docs/COMPATIBILITY.md:47-50`. W3.2B must target either a new adapter release
or a source-built artifact; it must not silently depend on APIs absent from the
published package.

### 10.2 Explicitly out of scope

- React Native support.
- Filesystem discovery, repository walking, or network acquisition.
- Persistence, caching, or telemetry.
- Reimplementing detection, validation, normalization, or transformation in
  TypeScript.
- Exposing `wasm-bindgen` classes or `free()`.
- Changing Core semantics for TypeScript convenience.

## 11. Implementation milestones

| Milestone | Deliverable | Acceptance |
| --- | --- | --- |
| **W3.2A** | Contract (this document): feature inventory, public API, lifecycle, offsets, errors, packaging, capability model | Reviewed and approved; no code |
| **W3.2B** | Implementation. Adapter work: version/capability exports, DTO materialization primitives, enum normalization. TypeScript package: `init`, synchronous `createScanner`, `scan`, `scanBatch`, transforms, Share Bundle, streaming, incremental, errors, offset helper | Type tests; Node ESM smoke test; explicit-asset browser smoke test; parity against direct WASM |
| **W3.2C** | Qualification, distribution, and documentation: browser/bundler matrix, published-tarball consumer, capability truthfulness, disposal and DTO-autonomy tests, user docs, release preparation | Each claimed capability and environment verified; unsupported combinations documented |
| **W3.3** | Native Rust MCP server (separate workstream; reuses Core, not this TypeScript package) | Separate design and approval |

W3.2B may deliver adapter exports and the wrapper incrementally, but it must not
claim a capability before its verification exists. W3.2C owns the transition
from "implemented" to "qualified and distributed". W3.3 is independent and does
not gate the TypeScript contract.

Each milestone must include a test proving that no returned DTO becomes invalid
after its producer is disposed, and that unsupported backend operations throw
`unsupported_capability` rather than disappearing.

## 12. Decisions and remaining questions

The following decisions are approved for W3.2A. "Resolved" means the contract
fixes the behavior; "Partially pending" means the contract fixes the shape but
one input still needs approval; "Pending" means the repository cannot yet verify
the answer.

| # | Question | Decision | Status |
| --- | --- | --- | --- |
| 1 | Package identity | Working name `@cribra/cribra`; final npm name and ownership are not verified | Pending |
| 2 | Module format | ESM-only for v1. CommonJS is out of scope and may be revisited after W3.2C | Resolved |
| 3 | Minimum runtime | Working assumption Node 20+; browser baseline and feature detection verified in W3.2C | Pending |
| 4 | Revision type | Keep `bigint`; never narrow to `number` | Resolved |
| 5 | Stored-result transforms | Scan-coupled transformations only; stored-result transforms deferred beyond v1 | Resolved |
| 6 | Multi-transform batching | One request per scan; multi-output batching deferred beyond v1 | Resolved |
| 7 | Capability scope | `getCapabilities()` is module-level and backend-level. Streaming admission is per-scanner/per-ruleset and reported by `stream_unsupported_ruleset` | Resolved |
| 8 | Custom rule exposure | Include `ScannerOptions.rules` in v1; the WASM builder already exports all four families | Resolved |
| 9 | Version sourcing | Adapter version and capability manifest are implemented in W3.2B-1. `core` stays absent unless an additive Core `VERSION` constant is approved | Partially pending: adapter resolved |
| 10 | Explanation cost | `includeExplanations` remains opt-in. A batched projection is an optimization, not a v1 requirement | Resolved |
| 11 | WASM asset delivery | Bundle `.wasm` in the npm tarball; callers may override with explicit bytes or a URL | Resolved |
| 12 | Capability encoding | Typed booleans. Unsupported operations use `unsupported_capability`; no per-field reason strings | Resolved |
| 13 | Streaming usability | Scanner construction never depends on admission; `stream()` reports `stream_unsupported_ruleset` and leaves the scanner usable | Resolved |

Genuinely pending items:

- npm package name, scope, and ownership (requires registry and maintainer
  confirmation).
- Approval of an additive `cribra::VERSION` constant, if `getVersion().core`
  should ever be non-optional.
- Node and browser baseline verification, including `WebAssembly`,
  `bigint`, and bundler asset handling, during W3.2C.

## Appendix: source references

Core:

- `src/scanner.rs` — scan, parallel scan, stream and incremental session creation
- `src/stream.rs` — streaming limits, session, push, finish
- `src/incremental_session.rs` — limits, edit protocol, revision, report
- `src/transform/mod.rs` — transformation exports and `TransformError`
- `src/transform/share_bundle.rs` — Share Bundle builder and data
- `src/location.rs` — byte span, line, scalar column
- `src/finding.rs` — finding projection and explanation
- `src/explanation.rs` — rule-metadata resolution
- `src/sensitive_candidate.rs` — candidates and evidence
- `src/scan_report.rs`, `src/scan_results.rs`, `src/scan_summary.rs` — reports
- `src/rule.rs`, `src/rule_metadata.rs`, `src/scanner_builder.rs` — rules
- `src/builtins/mod.rs`, `src/builtins/financial.rs`, `src/builtins/personal.rs` — catalogs

WASM adapter:

- `crates/cribra-wasm/src/lib.rs` — engine, builder, projections, transformations
- `crates/cribra-wasm/src/batch.rs` — sequential batch
- `crates/cribra-wasm/src/streaming.rs` — streaming session and limits
- `crates/cribra-wasm/src/incremental.rs` — incremental session and limits
- `crates/cribra-wasm/src/share_bundle.rs` — Share Bundle projection
- `crates/cribra-wasm/src/metadata.rs` — rule metadata
- `crates/cribra-wasm/src/build_error.rs` — construction error codes
- `crates/cribra-wasm/src/numeric.rs` — safe integer validation
- `crates/cribra-wasm/Cargo.toml` — profiles and features
- `crates/cribra-wasm/tests/runtime/` — runtime qualification and API contract

Project documentation:

- `docs/WASM_INTEROP.md` — WASM contract and profile behavior
- `docs/streaming.md` — streaming admission and finalization
- `docs/incremental.md` — revisioned document semantics
- `docs/COMPATIBILITY.md` — publication status
- `docs/ECOSYSTEM.md` — integration surface status
- `ROADMAP.md` — TypeScript workstream ordering
