/** Public, backend-neutral type surface. No generated wasm-bindgen type is exported. */

export type Backend = "wasm" | "native";
export type Profile = "core" | "flow";

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

export type Explanation =
  | { readonly kind: "classified"; readonly detectionMode: DetectionMode }
  | { readonly kind: "ambiguous"; readonly evidence: CandidateEvidence }
  | { readonly kind: "unknown" };

export interface ByteSpan {
  readonly start: number;
  readonly end: number;
}

/** Half-open UTF-16 code-unit range, suitable for `String.prototype.slice`. */
export interface TextRange {
  readonly start: number;
  readonly end: number;
}

export interface InitOptions {
  /** Explicit WASM bytes or URL. Omitted: resolve relative to the package asset. */
  readonly wasm?: Uint8Array | ArrayBuffer | URL | string;
}

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

export interface VersionInfo {
  /** Version of the TypeScript package itself. Always known. */
  readonly package: string;
  /** Cribra Core version, only when the backend can prove it. */
  readonly core?: string;
  /** WebAssembly adapter version, only when the backend can prove it. */
  readonly wasmAdapter?: string;
}

export interface ScannerOptions {
  /** Defaults to "current". "none" selects an empty built-in catalog. */
  readonly builtins?: "none" | "current";
  readonly financialBuiltins?: boolean;
  readonly personalBuiltins?: boolean;
  /** Custom rules, appended after selected built-ins. */
  readonly rules?: readonly RuleSpec[];
}

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
  /**
   * Reserved by the approved contract. The current WASM adapter exposes no
   * capture-aware builder entry, so a defined value is rejected with
   * `CribraError("unsupported_capability")`.
   */
  readonly captureGroup?: number;
}

export type RuleSpec =
  | LiteralRuleSpec
  | PrefixRuleSpec
  | SuffixRuleSpec
  | PatternRuleSpec;

export interface ScanOptions {
  /** Adds Finding.explanation. Default false because resolution is O(findings × rules). */
  readonly includeExplanations?: boolean;
}

export interface ScanSource {
  /** Preserved verbatim; may be empty, non-ASCII, or duplicated. */
  readonly id: string;
  readonly source: string;
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

export interface BatchEntry {
  readonly id: string;
  readonly sourceBytes: number;
  readonly result: ScanResult;
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

export interface BatchResult {
  /** Same length and order as the input array. */
  readonly entries: readonly BatchEntry[];
  readonly summary: ScanSummary;
}

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

  /** Consumes the session and returns the complete detached report. */
  finish(): ScanResult;

  /** Releases the session without a report. Idempotent. */
  dispose(): void;
}

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
   * Returns the next exact bigint revision.
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

export interface RuleMetadata {
  readonly ruleId: string;
  readonly kind: RuleKind;
  readonly detectionMode: DetectionMode;
  readonly severity: Severity;
  readonly remediation: Remediation | null;
}

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

export interface Scanner {
  readonly rulesCount: number;
  readonly rules: readonly RuleMetadata[];
  readonly disposed: boolean;

  scan(source: string, options?: ScanOptions): ScanResult;
  scanBatch(sources: readonly ScanSource[], options?: ScanOptions): BatchResult;

  /** Opens an admitted streaming session. The session outlives this scanner. */
  stream(options?: StreamOptions): StreamSession;

  /** Opens an editable UTF-8 document. The session outlives this scanner. */
  incremental(source: string, options?: IncrementalOptions): IncrementalSession;

  /** Idempotent. After disposal, scanning methods throw CribraError("disposed"). */
  dispose(): void;
}
