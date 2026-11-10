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
  ScanReportDto,
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
    default:
      return new CribraError('native_error', detail);
  }
}

function span(start: number, end: number): ByteSpan {
  return Object.freeze({ start, end });
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
      throw new CribraError('invalid_native_report');
  }
}

function confidenceFromNative(value: string): Confidence {
  switch (value) {
    case 'low':
    case 'medium':
    case 'high':
      return value;
    default:
      throw new CribraError('invalid_native_report');
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
      return 'unknown';
  }
}

function detectionModeFromNative(value: string): DetectionMode | undefined {
  switch (value) {
    case 'matcher_only':
    case 'deterministic':
    case 'contextual':
      return value;
    default:
      return undefined;
  }
}

function evidenceFromNative(value: string): CandidateEvidence {
  return value === 'structural' ? 'structural' : 'unknown';
}

function kindFromNative(value: string): CandidateKind {
  return value === 'recovery_like_code' ? 'recovery_like_code' : 'unknown';
}

function explanationFromNative(
  kind: string,
  value: string,
): Explanation {
  if (kind === 'classified') {
    const detectionMode = detectionModeFromNative(value);
    if (detectionMode !== undefined) {
      return Object.freeze({ kind: 'classified' as const, detectionMode });
    }
  }
  if (kind === 'ambiguous') {
    return Object.freeze({
      kind: 'ambiguous' as const,
      evidence: evidenceFromNative(value),
    });
  }
  return Object.freeze({ kind: 'unknown' as const });
}

function projectFinding(dto: FindingDto): Finding {
  return Object.freeze({
    ruleId: dto.ruleId,
    span: span(dto.start, dto.end),
    line: dto.line,
    column: dto.column,
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
    span: span(dto.start, dto.end),
    line: dto.line,
    column: dto.column,
    explanation: explanationFromNative(
      dto.explanationKind,
      dto.explanationValue,
    ),
  });
}

function projectReport(dto: ScanReportDto): ScanResult {
  const findings = Object.freeze(dto.findings.map(projectFinding));
  const candidates = Object.freeze(dto.candidates.map(projectCandidate));

  return Object.freeze({
    sourceBytes: dto.sourceBytes,
    findings,
    candidates,
    findingCount: findings.length,
    candidateCount: dto.candidateCount,
    needsReview: dto.needsReview,
    hasCritical: findings.some((finding) => finding.severity === 'critical'),
  });
}

function summarize(entries: readonly BatchEntry[]): ScanSummary {
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

  for (const entry of entries) {
    const { result } = entry;
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

  return Object.freeze({
    scannedSources: entries.length,
    scannedBytes,
    reportsWithFindings,
    reportsWithoutFindings: entries.length - reportsWithFindings,
    totalFindings,
    totalCandidates,
    reportsWithCandidates,
    hasCandidates: totalCandidates !== 0,
    critical,
    high,
    medium,
    low,
    info,
    isClean: totalFindings === 0 && totalCandidates === 0,
    hasCritical: critical !== 0,
  });
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
   * Scans sources in input order. React Native cannot expose the synchronous
   * WebAssembly batch call, so this awaits one native scan per source; the
   * native worker itself remains serial.
   */
  async scanBatch(
    sources: readonly ScanSource[],
  ): Promise<BatchResult> {
    this.#assertLive();
    if (!Array.isArray(sources)) {
      throw new CribraError('invalid_argument');
    }

    const entries: BatchEntry[] = [];
    for (const candidate of sources) {
      const source = requireSource(candidate);
      this.#assertLive();
      try {
        const dto = await NativeCribraReactNative.scan(
          this.#nativeId,
          source.id,
          source.source,
        );
        const result = projectReport(dto);
        entries.push(
          Object.freeze({
            id: source.id,
            sourceBytes: dto.sourceBytes,
            result,
          }),
        );
      } catch (error) {
        throw normalizeError(error);
      }
    }

    return Object.freeze({
      entries: Object.freeze(entries),
      summary: summarize(entries),
    });
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
