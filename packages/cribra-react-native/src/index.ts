/**
 * Pure TypeScript facade over the Cribra C ABI TurboModule.
 *
 * This is the native (React Native) backend. Unlike @metalymph/cribra's
 * WebAssembly backend, scanner creation, scanning, and disposal are inherently
 * asynchronous because native work runs on a serial worker and resolves on the
 * JS thread.
 */

import NativeCribraReactNative from './NativeCribraReactNative';
import type {
  CandidateDto,
  FindingDto,
  ScanBatchDto,
  ScanReportDto,
  ScanSourceDto,
  ScanSummaryDto,
} from './NativeCribraReactNative';

export type Severity = 'info' | 'low' | 'medium' | 'high' | 'critical';
export type Confidence = 'low' | 'medium' | 'high';
export type DetectionMode = 'matcher_only' | 'deterministic' | 'contextual';
export type CandidateKind = 'recovery_like_code' | 'unknown';
export type CandidateEvidence = 'structural' | 'unknown';

export type Remediation =
  | 'revoke_and_rotate_credential'
  | 'rotate_credential'
  | 'rotate_password'
  | 'replace_private_key'
  | 'remove_sensitive_value'
  | 'review_sensitive_hash'
  | 'review_password_verifier'
  | 'unknown';

export type Explanation =
  | { readonly kind: 'classified'; readonly detectionMode: DetectionMode }
  | { readonly kind: 'ambiguous'; readonly evidence: CandidateEvidence }
  | { readonly kind: 'unknown' };

export interface ByteSpan {
  readonly start: number;
  readonly end: number;
}

export interface AbiVersion {
  readonly major: number;
  readonly minor: number;
  readonly patch: number;
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
  /** The native backend always materialises explanations. */
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

export interface ScanOptions {
  /** Source identifier preserved verbatim in the native DTO. */
  readonly id?: string;
}

export type CribraErrorCode =
  | 'disposed'
  | 'invalid_argument'
  | 'invalid_input_string'
  | 'invalid_utf8'
  | 'scanner_not_found'
  | 'scanner_create_failed'
  | 'abi_mismatch'
  | 'invalid_native_report'
  | 'native_error';

const MESSAGES: Record<CribraErrorCode, string> = {
  disposed: 'The Cribra scanner has been disposed.',
  invalid_argument: 'A Cribra argument is invalid.',
  invalid_input_string: 'The source could not be converted to UTF-8.',
  invalid_utf8: 'The source is not valid UTF-8.',
  scanner_not_found: 'The native scanner handle is not available.',
  scanner_create_failed: 'The native scanner could not be created.',
  abi_mismatch: 'The linked Cribra native ABI is incompatible with this package.',
  invalid_native_report: 'The native scanner returned an unsupported value.',
  native_error: 'The Cribra native operation failed.',
};

/**
 * Structured error with a fixed, source-free message. `detail` carries only the
 * privacy-safe diagnostic metadata returned by the C ABI.
 */
export class CribraError extends Error {
  readonly code: CribraErrorCode;
  readonly detail?: string;

  constructor(code: CribraErrorCode, detail?: string) {
    super(MESSAGES[code]);
    this.name = 'CribraError';
    this.code = code;
    if (detail !== undefined && detail.length > 0) {
      this.detail = detail;
    }
  }
}

function normalizeError(error: unknown): CribraError {
  if (error instanceof CribraError) {
    return error;
  }

  const detail =
    error instanceof Error
      ? error.message
      : typeof error === 'string'
        ? error
        : '';
  const match = /^cribra_native_error:([a-z0-9_]+)/i.exec(detail);
  const nativeCode = match?.[1];

  switch (nativeCode) {
    case 'invalid_argument':
      return new CribraError('invalid_argument', detail);
    case 'invalid_utf8':
      return new CribraError('invalid_utf8', detail);
    case 'scanner_not_found':
      return new CribraError('scanner_not_found', detail);
    case 'invalid_input_string':
      return new CribraError('invalid_input_string', detail);
    case 'abi_mismatch':
      return new CribraError('abi_mismatch', detail);
    case 'invalid_native_report':
    case 'numeric_overflow':
      return new CribraError('invalid_native_report', detail);
    default:
      return new CribraError('native_error', detail);
  }
}

function invalidNativeReport(field: string): CribraError {
  return new CribraError(
    'invalid_native_report',
    `cribra_native_error:invalid_native_report:${field}`,
  );
}

function safeInteger(value: number, field: string): number {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw invalidNativeReport(field);
  }
  return value;
}

function positiveInteger(value: number, field: string): number {
  const result = safeInteger(value, field);
  if (result < 1) {
    throw invalidNativeReport(field);
  }
  return result;
}

function safeSpan(start: number, end: number, field: string): ByteSpan {
  const safeStart = safeInteger(start, `${field}.start`);
  const safeEnd = safeInteger(end, `${field}.end`);
  if (safeEnd < safeStart) {
    throw invalidNativeReport(`${field}.span`);
  }
  return Object.freeze({ start: safeStart, end: safeEnd });
}

function severityFromNative(value: string): Severity {
  switch (value) {
    case 'info':
    case 'low':
    case 'medium':
    case 'high':
    case 'critical':
      return value;
    default:
      throw invalidNativeReport('finding.severity');
  }
}

function confidenceFromNative(value: string): Confidence {
  switch (value) {
    case 'low':
    case 'medium':
    case 'high':
      return value;
    default:
      throw invalidNativeReport('finding.confidence');
  }
}

function remediationFromNative(value: string): Remediation | null {
  if (value === '') {
    return null;
  }
  switch (value) {
    case 'revoke_and_rotate_credential':
    case 'rotate_credential':
    case 'rotate_password':
    case 'replace_private_key':
    case 'remove_sensitive_value':
    case 'review_sensitive_hash':
    case 'review_password_verifier':
    case 'unknown':
      return value;
    default:
      throw invalidNativeReport('finding.remediation');
  }
}

function detectionModeFromNative(value: string): DetectionMode | undefined {
  switch (value) {
    case 'matcher_only':
    case 'deterministic':
    case 'contextual':
      return value;
    case 'none':
    case 'unknown':
    case '':
      return undefined;
    default:
      throw invalidNativeReport('explanation.detectionMode');
  }
}

function evidenceFromNative(value: string): CandidateEvidence {
  switch (value) {
    case 'structural':
      return 'structural';
    case 'none':
    case 'unknown':
    case '':
      return 'unknown';
    default:
      throw invalidNativeReport('candidate.evidence');
  }
}

function kindFromNative(value: string): CandidateKind {
  switch (value) {
    case 'recovery_like_code':
      return 'recovery_like_code';
    case 'none':
    case 'unknown':
    case '':
      return 'unknown';
    default:
      throw invalidNativeReport('candidate.kind');
  }
}

function explanationFromNative(
  kind: string,
  value: string,
): Explanation {
  switch (kind) {
    case 'classified': {
      const detectionMode = detectionModeFromNative(value);
      if (detectionMode === undefined) {
        // A classified explanation needs a concrete authority.
        throw invalidNativeReport('explanation.detectionMode');
      }
      return Object.freeze({ kind: 'classified' as const, detectionMode });
    }
    case 'ambiguous': {
      if (value === 'none' || value === '') {
        // An ambiguous explanation needs concrete evidence.
        throw invalidNativeReport('explanation.evidence');
      }
      return Object.freeze({
        kind: 'ambiguous' as const,
        evidence: evidenceFromNative(value),
      });
    }
    case 'none':
    case 'unknown':
    case '':
      if (value !== '') {
        // A payload in a context that has none is an impossible combination.
        throw invalidNativeReport('explanation.value');
      }
      return Object.freeze({ kind: 'unknown' as const });
    default:
      throw invalidNativeReport('explanation.kind');
  }
}

function requireArray<T>(value: readonly T[], field: string): readonly T[] {
  if (!Array.isArray(value)) {
    throw invalidNativeReport(field);
  }
  return value;
}

function projectFinding(dto: FindingDto): Finding {
  return Object.freeze({
    ruleId: dto.ruleId,
    span: safeSpan(dto.start, dto.end, 'finding'),
    line: positiveInteger(dto.line, 'finding.line'),
    column: positiveInteger(dto.column, 'finding.column'),
    severity: severityFromNative(dto.severity),
    confidence: confidenceFromNative(dto.confidence),
    remediation: remediationFromNative(dto.remediation),
    explanation: explanationFromNative(
      dto.explanationKind,
      dto.explanationValue,
    ),
  });
}

function projectCandidate(dto: CandidateDto): Candidate {
  return Object.freeze({
    kind: kindFromNative(dto.kind),
    evidence: evidenceFromNative(dto.evidence),
    span: safeSpan(dto.start, dto.end, 'candidate'),
    line: positiveInteger(dto.line, 'candidate.line'),
    column: positiveInteger(dto.column, 'candidate.column'),
    explanation: explanationFromNative(
      dto.explanationKind,
      dto.explanationValue,
    ),
  });
}

function projectReport(dto: ScanReportDto): ScanResult {
  if (typeof dto !== 'object' || dto === null) {
    throw invalidNativeReport('report');
  }
  const findings = Object.freeze(
    requireArray(dto.findings, 'findings').map(projectFinding),
  );
  const candidates = Object.freeze(
    requireArray(dto.candidates, 'candidates').map(projectCandidate),
  );
  const candidateCount = safeInteger(dto.candidateCount, 'candidateCount');
  if (candidateCount !== candidates.length) {
    throw invalidNativeReport('candidateCount');
  }

  return Object.freeze({
    sourceBytes: safeInteger(dto.sourceBytes, 'sourceBytes'),
    findings,
    candidates,
    findingCount: findings.length,
    candidateCount,
    needsReview: dto.needsReview,
    hasCritical: findings.some((finding) => finding.severity === 'critical'),
  });
}

function invariant(condition: boolean, field: string): void {
  if (!condition) {
    throw invalidNativeReport(field);
  }
}

function projectSummary(dto: ScanSummaryDto): ScanSummary {
  if (typeof dto !== 'object' || dto === null) {
    throw invalidNativeReport('summary');
  }
  const summary: ScanSummary = Object.freeze({
    scannedSources: safeInteger(dto.scannedSources, 'summary.scannedSources'),
    scannedBytes: safeInteger(dto.scannedBytes, 'summary.scannedBytes'),
    reportsWithFindings: safeInteger(
      dto.reportsWithFindings,
      'summary.reportsWithFindings',
    ),
    reportsWithoutFindings: safeInteger(
      dto.reportsWithoutFindings,
      'summary.reportsWithoutFindings',
    ),
    totalFindings: safeInteger(dto.totalFindings, 'summary.totalFindings'),
    totalCandidates: safeInteger(
      dto.totalCandidates,
      'summary.totalCandidates',
    ),
    reportsWithCandidates: safeInteger(
      dto.reportsWithCandidates,
      'summary.reportsWithCandidates',
    ),
    hasCandidates: dto.hasCandidates,
    critical: safeInteger(dto.critical, 'summary.critical'),
    high: safeInteger(dto.high, 'summary.high'),
    medium: safeInteger(dto.medium, 'summary.medium'),
    low: safeInteger(dto.low, 'summary.low'),
    info: safeInteger(dto.info, 'summary.info'),
    isClean: dto.isClean,
    hasCritical: dto.hasCritical,
  });

  // Integrity checks only: the native batch boundary remains the sole
  // aggregator and these values never reclassify findings.
  invariant(
    summary.reportsWithFindings + summary.reportsWithoutFindings ===
      summary.scannedSources,
    'summary.reportCounts',
  );
  invariant(
    summary.reportsWithFindings <= summary.scannedSources &&
      summary.reportsWithCandidates <= summary.scannedSources,
    'summary.reportCounts',
  );
  invariant(
    summary.critical +
      summary.high +
      summary.medium +
      summary.low +
      summary.info ===
      summary.totalFindings,
    'summary.severityTotals',
  );
  invariant(
    summary.hasCandidates === (summary.totalCandidates !== 0),
    'summary.hasCandidates',
  );
  invariant(
    summary.hasCritical === (summary.critical !== 0),
    'summary.hasCritical',
  );
  invariant(
    summary.isClean ===
      (summary.totalFindings === 0 && summary.totalCandidates === 0),
    'summary.isClean',
  );

  return summary;
}

function projectBatch(
  dto: ScanBatchDto,
  expectedIds: readonly string[],
): BatchResult {
  const reports = requireArray(dto.entries, 'entries');
  // The native boundary must echo the input order and identifiers verbatim.
  invariant(reports.length === expectedIds.length, 'entries.length');

  const entries = Object.freeze(
    reports.map((report, index) => {
      if (typeof report !== 'object' || report === null) {
        throw invalidNativeReport('entries');
      }
      invariant(report.sourceId === expectedIds[index], 'entries.id');
      const result = projectReport(report);
      return Object.freeze({
        id: report.sourceId,
        sourceBytes: result.sourceBytes,
        result,
      });
    }),
  );
  const summary = projectSummary(dto.summary);
  invariant(summary.scannedSources === entries.length, 'summary.scannedSources');

  // Cross-check the native summary against the projected entries. This detects
  // an aggregation mismatch without recomputing or reclassifying anything.
  let scannedBytes = 0;
  let reportsWithFindings = 0;
  let reportsWithCandidates = 0;
  let totalFindings = 0;
  let totalCandidates = 0;
  let critical = 0;
  let high = 0;
  let medium = 0;
  let low = 0;
  let info = 0;
  for (const { result } of entries) {
    scannedBytes += result.sourceBytes;
    totalFindings += result.findings.length;
    totalCandidates += result.candidateCount;
    if (result.findings.length > 0) {
      reportsWithFindings += 1;
    }
    if (result.candidateCount > 0) {
      reportsWithCandidates += 1;
    }
    for (const finding of result.findings) {
      switch (finding.severity) {
        case 'critical':
          critical += 1;
          break;
        case 'high':
          high += 1;
          break;
        case 'medium':
          medium += 1;
          break;
        case 'low':
          low += 1;
          break;
        case 'info':
          info += 1;
          break;
      }
    }
  }
  invariant(scannedBytes === summary.scannedBytes, 'summary.scannedBytes');
  invariant(
    reportsWithFindings === summary.reportsWithFindings,
    'summary.reportsWithFindings',
  );
  invariant(
    reportsWithCandidates === summary.reportsWithCandidates,
    'summary.reportsWithCandidates',
  );
  invariant(totalFindings === summary.totalFindings, 'summary.totalFindings');
  invariant(
    totalCandidates === summary.totalCandidates,
    'summary.totalCandidates',
  );
  invariant(critical === summary.critical, 'summary.critical');
  invariant(high === summary.high, 'summary.high');
  invariant(medium === summary.medium, 'summary.medium');
  invariant(low === summary.low, 'summary.low');
  invariant(info === summary.info, 'summary.info');

  return Object.freeze({ entries, summary });
}

function requireSource(source: unknown): ScanSource {
  if (
    typeof source !== 'object' ||
    source === null ||
    typeof (source as ScanSource).id !== 'string' ||
    typeof (source as ScanSource).source !== 'string'
  ) {
    throw new CribraError('invalid_argument');
  }
  return source as ScanSource;
}

/**
 * Reads the released Cribra C ABI version synchronously.
 */
export function getAbiVersion(): AbiVersion {
  return Object.freeze(NativeCribraReactNative.getAbiVersion());
}

/**
 * Diagnostic count of live native scanner handles owned by this module.
 */
export function getLiveScannerCount(): number {
  return NativeCribraReactNative.getLiveScannerCount();
}

/**
 * Creates one native scanner over Cribra's current built-in catalog.
 * Construction is asynchronous because it runs on the native worker.
 */
export async function createScanner(): Promise<CribraScanner> {
  return CribraScanner.create();
}

export class CribraScanner {
  readonly #nativeId: number;
  #disposed = false;
  #disposePromise: Promise<void> | undefined;

  private constructor(nativeId: number) {
    this.#nativeId = nativeId;
  }

  static async create(): Promise<CribraScanner> {
    try {
      const nativeId = await NativeCribraReactNative.createDefaultScanner();
      return new CribraScanner(nativeId);
    } catch (error) {
      const normalized = normalizeError(error);
      // An incompatible native ABI must fail closed with its own code, not be
      // folded into the generic scanner-construction failure.
      if (normalized.code === 'abi_mismatch') {
        throw normalized;
      }
      throw new CribraError('scanner_create_failed', normalized.detail);
    }
  }

  get disposed(): boolean {
    return this.#disposed;
  }

  /**
   * Scans one source. The native backend always materialises findings and
   * candidate explanations; there is no includeExplanations option.
   */
  async scan(source: string, options: ScanOptions = {}): Promise<ScanResult> {
    this.#assertLive();
    if (typeof source !== 'string') {
      throw new CribraError('invalid_argument');
    }
    const id = options.id ?? '';
    try {
      const dto = await NativeCribraReactNative.scan(
        this.#nativeId,
        id,
        source,
      );
      return projectReport(dto);
    } catch (error) {
      throw normalizeError(error);
    }
  }

  /**
   * Scans sources in input order through one native batch operation.
   *
   * Input is validated and copied before the native call, one worker job runs
   * N serial `cribra_scanner_scan` calls, and the whole batch resolves or
   * rejects atomically. Empty IDs, duplicate IDs, empty sources, and input
   * order are preserved.
   */
  async scanBatch(
    sources: readonly ScanSource[],
  ): Promise<BatchResult> {
    this.#assertLive();
    if (!Array.isArray(sources)) {
      throw new CribraError('invalid_argument');
    }

    // Validate every entry on the JS thread before dispatching any native work.
    const inputs: ScanSourceDto[] = [];
    for (const candidate of sources) {
      const source = requireSource(candidate);
      inputs.push({ id: source.id, source: source.source });
    }

    try {
      const dto = await NativeCribraReactNative.scanBatch(
        this.#nativeId,
        inputs,
      );
      return projectBatch(
        dto,
        inputs.map((input) => input.id),
      );
    } catch (error) {
      throw normalizeError(error);
    }
  }

  /**
   * Idempotent. Concurrent callers await the same native disposal, and a
   * failed disposal can be retried without re-enabling scans. After disposal,
   * scan methods throw CribraError("disposed").
   */
  async dispose(): Promise<void> {
    if (this.#disposePromise !== undefined) {
      return this.#disposePromise;
    }
    this.#disposed = true;
    const disposal = this.#disposeNative().catch((error: unknown) => {
      // Allow a later dispose() to retry a real native failure.
      this.#disposePromise = undefined;
      throw error;
    });
    this.#disposePromise = disposal;
    return disposal;
  }

  async #disposeNative(): Promise<void> {
    try {
      await NativeCribraReactNative.disposeScanner(this.#nativeId);
    } catch (error) {
      const normalized = normalizeError(error);
      if (normalized.code !== 'scanner_not_found') {
        throw normalized;
      }
    }
  }

  #assertLive(): void {
    if (this.#disposed) {
      throw new CribraError('disposed');
    }
  }
}
