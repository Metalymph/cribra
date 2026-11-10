/**
 * Facade-level RN.1.2 batch tests.
 *
 * The native TurboModule is mocked, so these tests pin the JavaScript contract:
 * one native call per batch, copied inputs, strict DTO/summary validation, and
 * source-free error normalisation. Native behaviour is exercised on the
 * simulator/emulator harness.
 */

jest.mock('../../src/NativeCribraReactNative', () => ({
  __esModule: true,
  default: {
    getAbiVersion: jest.fn(() => ({ major: 0, minor: 3, patch: 0 })),
    createDefaultScanner: jest.fn(),
    createScanner: jest.fn(),
    scan: jest.fn(),
    scanBatch: jest.fn(),
    disposeScanner: jest.fn(),
    getLiveScannerCount: jest.fn(),
  },
}));

import NativeCribraReactNative from '../../src/NativeCribraReactNative';
import {
  CribraError,
  CribraScanner,
  type BatchResult,
} from '../../src/index';
import type {
  CandidateDto,
  FindingDto,
  ScanBatchDto,
  ScanReportDto,
  ScanSourceDto,
  ScanSummaryDto,
  ScannerCreationDto,
} from '../../src/NativeCribraReactNative';

const native = NativeCribraReactNative as unknown as {
  createScanner: jest.Mock;
  scanBatch: jest.Mock;
  disposeScanner: jest.Mock;
};

const scannerId = 7;

function creationDto(): ScannerCreationDto {
  return {
    scannerId,
    rules: [],
    failure: '',
    buildCode: 0,
    ruleId: '',
    detail: '',
  };
}

function finding(overrides: Partial<FindingDto> = {}): FindingDto {
  return {
    ruleId: 'github.classic-pat',
    start: 0,
    end: 8,
    line: 1,
    column: 1,
    severity: 'critical',
    confidence: 'high',
    remediation: 'revoke_and_rotate_credential',
    explanationKind: 'classified',
    explanationValue: 'deterministic',
    ...overrides,
  };
}

function candidate(overrides: Partial<CandidateDto> = {}): CandidateDto {
  return {
    kind: 'recovery_like_code',
    start: 0,
    end: 4,
    line: 1,
    column: 1,
    evidence: 'structural',
    explanationKind: 'ambiguous',
    explanationValue: 'structural',
    ...overrides,
  };
}

function report(
  sourceId: string,
  overrides: Partial<ScanReportDto> = {},
): ScanReportDto {
  return {
    sourceId,
    sourceBytes: 8,
    findings: [],
    candidateCount: 0,
    needsReview: false,
    candidates: [],
    ...overrides,
  };
}

function zeroSummary(): ScanSummaryDto {
  return {
    scannedSources: 0,
    scannedBytes: 0,
    reportsWithFindings: 0,
    reportsWithoutFindings: 0,
    totalFindings: 0,
    totalCandidates: 0,
    reportsWithCandidates: 0,
    hasCandidates: false,
    critical: 0,
    high: 0,
    medium: 0,
    low: 0,
    info: 0,
    isClean: true,
    hasCritical: false,
  };
}

function summaryFor(entries: readonly ScanReportDto[]): ScanSummaryDto {
  const summary = zeroSummary();
  summary.scannedSources = entries.length;
  for (const entry of entries) {
    summary.scannedBytes += entry.sourceBytes;
    summary.totalFindings += entry.findings.length;
    summary.totalCandidates += entry.candidateCount;
    if (entry.findings.length > 0) {
      summary.reportsWithFindings += 1;
    }
    if (entry.candidateCount > 0) {
      summary.reportsWithCandidates += 1;
    }
    for (const item of entry.findings) {
      switch (item.severity) {
        case 'critical':
          summary.critical += 1;
          break;
        case 'high':
          summary.high += 1;
          break;
        case 'medium':
          summary.medium += 1;
          break;
        case 'low':
          summary.low += 1;
          break;
        case 'info':
          summary.info += 1;
          break;
      }
    }
  }
  summary.reportsWithoutFindings =
    summary.scannedSources - summary.reportsWithFindings;
  summary.hasCandidates = summary.totalCandidates !== 0;
  summary.isClean =
    summary.totalFindings === 0 && summary.totalCandidates === 0;
  summary.hasCritical = summary.critical !== 0;
  return summary;
}

function batchOf(entries: ScanReportDto[]): ScanBatchDto {
  return { entries, summary: summaryFor(entries) };
}

async function createScanner(): Promise<CribraScanner> {
  return CribraScanner.create();
}

async function expectCode(
  promise: Promise<unknown>,
  code: CribraError['code'],
): Promise<void> {
  await expect(promise).rejects.toMatchObject({ name: 'CribraError', code });
}

beforeEach(() => {
  jest.clearAllMocks();
  native.createScanner.mockResolvedValue(creationDto());
  native.disposeScanner.mockResolvedValue(undefined);
});

describe('scanBatch dispatch', () => {
  test('uses exactly one native call with copied, ordered inputs', async () => {
    const scanner = await createScanner();
    const entries = [
      report('a', { sourceBytes: 3 }),
      report('', { sourceBytes: 0 }),
      report('a', { sourceBytes: 4 }),
      report('dup', { sourceBytes: 4 }),
    ];
    native.scanBatch.mockResolvedValue(batchOf(entries));

    const inputs: ScanSourceDto[] = [
      { id: 'a', source: 'abc' },
      { id: '', source: '' },
      { id: 'a', source: 'é🙂' },
      { id: 'dup', source: 'text' },
    ];
    const result = await scanner.scanBatch(inputs);

    expect(native.scanBatch).toHaveBeenCalledTimes(1);
    expect(native.scanBatch).toHaveBeenCalledWith(scannerId, inputs);
    // A fresh array is handed to native; later input mutation cannot race.
    expect(native.scanBatch.mock.calls[0][1]).not.toBe(inputs);
    expect(result.entries.map((entry) => entry.id)).toEqual([
      'a',
      '',
      'a',
      'dup',
    ]);
    expect(result.summary.scannedSources).toBe(4);
    expect(result.summary.scannedBytes).toBe(11);
  });

  test('an empty batch is one native call with an accurate zero summary', async () => {
    const scanner = await createScanner();
    native.scanBatch.mockResolvedValue({
      entries: [],
      summary: zeroSummary(),
    });

    const result = await scanner.scanBatch([]);

    expect(native.scanBatch).toHaveBeenCalledTimes(1);
    expect(native.scanBatch).toHaveBeenCalledWith(scannerId, []);
    expect(result.entries).toEqual([]);
    expect(result.summary).toEqual(zeroSummary());
  });

  test('keeps the full projection, including findings, candidates, and explanations', async () => {
    const scanner = await createScanner();
    const entries = [
      report('rich', {
        sourceBytes: 12,
        findings: [finding(), finding({ ruleId: 'stripe.test-secret-key', severity: 'high' })],
        candidates: [candidate()],
        candidateCount: 1,
        needsReview: true,
      }),
    ];
    native.scanBatch.mockResolvedValue(batchOf(entries));

    const result = await scanner.scanBatch([{ id: 'rich', source: 'ignored' }]);

    expect(result.entries[0].result).toMatchObject({
      sourceBytes: 12,
      findingCount: 2,
      candidateCount: 1,
      needsReview: true,
      hasCritical: true,
    });
    expect(result.entries[0].result.findings[0]).toEqual({
      ruleId: 'github.classic-pat',
      span: { start: 0, end: 8 },
      line: 1,
      column: 1,
      severity: 'critical',
      confidence: 'high',
      remediation: 'revoke_and_rotate_credential',
      explanation: { kind: 'classified', detectionMode: 'deterministic' },
    });
    expect(result.entries[0].result.candidates[0]).toEqual({
      kind: 'recovery_like_code',
      evidence: 'structural',
      span: { start: 0, end: 4 },
      line: 1,
      column: 1,
      explanation: { kind: 'ambiguous', evidence: 'structural' },
    });
    expect(result.summary).toMatchObject({
      totalFindings: 2,
      totalCandidates: 1,
      reportsWithFindings: 1,
      reportsWithCandidates: 1,
      critical: 1,
      high: 1,
    });
  });

  test('rejects malformed JavaScript input before native work', async () => {
    const scanner = await createScanner();

    await expectCode(
      scanner.scanBatch('not-an-array' as unknown as ScanSourceDto[]),
      'invalid_argument',
    );
    await expectCode(
      scanner.scanBatch([
        { id: 'a' },
      ] as unknown as ScanSourceDto[]),
      'invalid_argument',
    );
    await expectCode(
      scanner.scanBatch([
        { id: 1, source: 'x' },
      ] as unknown as ScanSourceDto[]),
      'invalid_argument',
    );

    expect(native.scanBatch).not.toHaveBeenCalled();
  });

  test('a disposed facade rejects without native work', async () => {
    const scanner = await createScanner();
    await scanner.dispose();

    await expectCode(scanner.scanBatch([]), 'disposed');
    expect(native.scanBatch).not.toHaveBeenCalled();
  });

  test('disposal does not cancel an already-dispatched batch', async () => {
    const scanner = await createScanner();
    let resolveBatch!: (value: ScanBatchDto) => void;
    native.scanBatch.mockReturnValue(
      new Promise<ScanBatchDto>((resolve) => {
        resolveBatch = resolve;
      }),
    );

    const pending = scanner.scanBatch([]);
    const disposing = scanner.dispose();
    resolveBatch({ entries: [], summary: zeroSummary() });

    await expect(pending).resolves.toMatchObject({
      entries: [],
      summary: zeroSummary(),
    });
    await disposing;
    expect(native.disposeScanner).toHaveBeenCalledTimes(1);
  });
});

describe('strict native DTO validation', () => {
  async function rejects(
    entry: ScanReportDto,
    inputId = entry.sourceId,
  ): Promise<CribraError> {
    const scanner = await createScanner();
    native.scanBatch.mockResolvedValue(batchOf([entry]));
    try {
      await scanner.scanBatch([{ id: inputId, source: 'ignored' }]);
      throw new Error('expected rejection');
    } catch (error) {
      expect(error).toBeInstanceOf(CribraError);
      return error as CribraError;
    }
  }

  test('rejects an unknown severity', async () => {
    const error = await rejects(
      report('a', { findings: [finding({ severity: 'severe' })] }),
    );
    expect(error.code).toBe('invalid_native_report');
    expect(error.detail).toBe(
      'cribra_native_error:invalid_native_report:finding.severity',
    );
  });

  test('rejects unknown confidence and remediation', async () => {
    expect(
      (
        await rejects(
          report('a', { findings: [finding({ confidence: 'certain' })] }),
        )
      ).detail,
    ).toContain('finding.confidence');
    expect(
      (
        await rejects(
          report('a', { findings: [finding({ remediation: 'rotate' })] }),
        )
      ).detail,
    ).toContain('finding.remediation');
  });

  test('rejects impossible explanation kind/payload combinations', async () => {
    expect(
      (
        await rejects(
          report('a', {
            findings: [
              finding({
                explanationKind: 'classified',
                explanationValue: 'none',
              }),
            ],
          }),
        )
      ).detail,
    ).toContain('explanation.detectionMode');
    expect(
      (
        await rejects(
          report('a', {
            findings: [
              finding({ explanationKind: 'ambiguous', explanationValue: '' }),
            ],
          }),
        )
      ).detail,
    ).toContain('explanation.evidence');
    expect(
      (
        await rejects(
          report('a', {
            findings: [
              finding({ explanationKind: 'mystery', explanationValue: '' }),
            ],
          }),
        )
      ).detail,
    ).toContain('explanation.kind');
  });

  test('rejects out-of-range and imprecise coordinates', async () => {
    expect(
      (
        await rejects(
          report('a', { findings: [finding({ line: 0 })] }),
        )
      ).detail,
    ).toContain('finding.line');
    expect(
      (
        await rejects(
          report('a', {
            findings: [
              finding({ start: Number.MAX_SAFE_INTEGER + 1 }),
            ],
          }),
        )
      ).detail,
    ).toContain('finding.start');
    expect(
      (
        await rejects(
          report('a', {
            findings: [finding({ start: 9, end: 3 })],
          }),
        )
      ).detail,
    ).toContain('finding.span');
    expect(
      (await rejects(report('a', { sourceBytes: -1 }))).detail,
    ).toContain('sourceBytes');
  });

  test('rejects a candidate count that disagrees with the array', async () => {
    const error = await rejects(
      report('a', {
        candidateCount: 2,
        candidates: [candidate()],
      }),
    );
    expect(error.detail).toContain('candidateCount');
  });

  test('rejects a native summary that disagrees with the entries', async () => {
    const scanner = await createScanner();
    const entries = [report('a')];
    const summary = summaryFor(entries);
    summary.totalFindings = 1;
    native.scanBatch.mockResolvedValue({ entries, summary });

    await expectCode(scanner.scanBatch([{ id: 'a', source: 'x' }]), 'invalid_native_report');
  });

  test('rejects an entry id that does not echo the input', async () => {
    const scanner = await createScanner();
    native.scanBatch.mockResolvedValue(batchOf([report('other')]));

    await expectCode(
      scanner.scanBatch([{ id: 'a', source: 'x' }]),
      'invalid_native_report',
    );
  });

  test('rejects a summary with impossible report counts', async () => {
    const scanner = await createScanner();
    const summary = zeroSummary();
    summary.scannedSources = 1;
    summary.reportsWithFindings = 1;
    summary.reportsWithoutFindings = 1;
    native.scanBatch.mockResolvedValue({ entries: [], summary });

    await expectCode(scanner.scanBatch([]), 'invalid_native_report');
  });
});

describe('error normalisation', () => {
  test.each([
    ['cribra_native_error:invalid_argument', 'invalid_argument'],
    ['cribra_native_error:invalid_utf8:scan', 'invalid_utf8'],
    ['cribra_native_error:invalid_input_string', 'invalid_input_string'],
    ['cribra_native_error:scanner_not_found', 'scanner_not_found'],
    [
      'cribra_native_error:invalid_native_report:summary.critical:7',
      'invalid_native_report',
    ],
    [
      'cribra_native_error:numeric_overflow:summary.totalFindings',
      'invalid_native_report',
    ],
  ] as const)('maps %s to %s', async (message, code) => {
    const scanner = await createScanner();
    native.scanBatch.mockRejectedValue(new Error(message));

    await expect(scanner.scanBatch([])).rejects.toMatchObject({
      name: 'CribraError',
      code,
      detail: message,
    });
  });

  test('does not preserve an arbitrary native exception message as a code', async () => {
    const scanner = await createScanner();
    native.scanBatch.mockRejectedValue(new Error('boom'));
    const result = scanner.scanBatch([]);
    await expectCode(result, 'native_error');
  });
});
