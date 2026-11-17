/**
 * RN.1.5 native Flow transformation facade tests.
 *
 * The TurboModule is mocked, so these tests pin the JavaScript contract:
 * request normalization, Core defaults, key copying/zeroization, UTF-16
 * preflight, one native call per transform, DTO projection, and the exact
 * compatibility classification of fixed C ABI transform diagnostics.
 */

jest.mock('../../src/NativeCribraReactNative', () => ({
  __esModule: true,
  default: {
    getAbiVersion: jest.fn(() => ({ major: 0, minor: 3, patch: 0 })),
    createDefaultScanner: jest.fn(),
    createScanner: jest.fn(),
    scan: jest.fn(),
    scanBatch: jest.fn(),
    transformRedact: jest.fn(),
    transformTemplate: jest.fn(),
    transformPseudonymize: jest.fn(),
    transformSynthesize: jest.fn(),
    disposeScanner: jest.fn(),
    getLiveScannerCount: jest.fn(() => 0),
  },
}));

import NativeCribraReactNative from '../../src/NativeCribraReactNative';
import { createScanner, type CribraScanner } from '../../src/index';
import type {
  ScanReportDto,
  ScannerCreationDto,
  TransformResultDto,
} from '../../src/NativeCribraReactNative';

const native = NativeCribraReactNative as unknown as {
  createScanner: jest.Mock;
  scan: jest.Mock;
  transformRedact: jest.Mock;
  transformTemplate: jest.Mock;
  transformPseudonymize: jest.Mock;
  transformSynthesize: jest.Mock;
  disposeScanner: jest.Mock;
};

const scannerId = 23;

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

function transformDto(
  overrides: Partial<TransformResultDto> = {},
): TransformResultDto {
  return {
    sourceBytes: 6,
    findings: [],
    candidateCount: 0,
    needsReview: false,
    candidates: [],
    output: 'output',
    ...overrides,
  };
}

function scanDto(): ScanReportDto {
  return {
    sourceId: 'doc',
    sourceBytes: 6,
    findings: [],
    candidateCount: 0,
    needsReview: false,
    candidates: [],
  };
}

const KEY = Uint8Array.from({ length: 32 }, (_value, index) => index);
const KEY_OTHER = Uint8Array.from(
  { length: 32 },
  (_value, index) => 255 - index,
);

let scanner: CribraScanner;

beforeEach(async () => {
  native.createScanner.mockResolvedValue(creationDto());
  native.scan.mockResolvedValue(scanDto());
  native.transformRedact.mockReset();
  native.transformTemplate.mockReset();
  native.transformPseudonymize.mockReset();
  native.transformSynthesize.mockReset();
  native.transformRedact.mockResolvedValue(transformDto());
  native.transformTemplate.mockResolvedValue(transformDto());
  native.transformPseudonymize.mockResolvedValue(transformDto());
  native.transformSynthesize.mockResolvedValue(transformDto());
  scanner = await createScanner();
});

describe('request normalization and Core defaults', () => {
  it('dispatches one redact call and defaults to Core replacement', async () => {
    const result = await scanner.transform('source', { kind: 'redact' });

    expect(native.transformRedact).toHaveBeenCalledTimes(1);
    expect(native.transformRedact).toHaveBeenCalledWith(
      scannerId,
      'source',
      '',
      false,
    );
    expect(result.output).toBe('output');
    expect(Object.isFrozen(result)).toBe(true);
  });

  it('distinguishes an explicit empty redaction replacement', async () => {
    await scanner.transform('source', { kind: 'redact', replacement: '' });

    expect(native.transformRedact).toHaveBeenCalledWith(
      scannerId,
      'source',
      '',
      true,
    );
  });

  it('resolves template defaults and custom options', async () => {
    await scanner.transform('source', { kind: 'template' });
    await scanner.transform('source', {
      kind: 'template',
      namespace: 'ACME',
      numbered: true,
    });

    expect(native.transformTemplate).toHaveBeenNthCalledWith(
      1,
      scannerId,
      'source',
      'CRIBRA',
      false,
    );
    expect(native.transformTemplate).toHaveBeenNthCalledWith(
      2,
      scannerId,
      'source',
      'ACME',
      true,
    );
  });

  it('resolves pseudonymize defaults and forwards custom options', async () => {
    const copies: Uint8Array[] = [];
    native.transformPseudonymize.mockImplementation(
      async (_id, _text, buffer: ArrayBuffer, _prefix, _digest) => {
        copies.push(new Uint8Array(buffer).slice());
        return transformDto();
      },
    );

    await scanner.transform('source', { kind: 'pseudonymize', key: KEY });
    await scanner.transform('source', {
      kind: 'pseudonymize',
      key: KEY,
      prefix: 'acme_',
      digestBytes: 8,
    });

    const first = native.transformPseudonymize.mock.calls[0];
    expect(first[0]).toBe(scannerId);
    expect(first[1]).toBe('source');
    expect(first[2]).toBeInstanceOf(ArrayBuffer);
    expect(first[2].byteLength).toBe(32);
    expect([...copies[0]]).toEqual([...KEY]);
    expect(first[3]).toBe('cribra_pseudo_');
    expect(first[4]).toBe(16);

    const second = native.transformPseudonymize.mock.calls[1];
    expect(second[3]).toBe('acme_');
    expect(second[4]).toBe(8);
  });

  it('resolves synthesize defaults and forwards a custom marker', async () => {
    await scanner.transform('source', { kind: 'synthesize', key: KEY });
    await scanner.transform('source', {
      kind: 'synthesize',
      key: KEY,
      marker: 'acme_synthetic',
    });

    expect(native.transformSynthesize.mock.calls[0][3]).toBe('cribra_synthetic');
    expect(native.transformSynthesize.mock.calls[1][3]).toBe('acme_synthetic');
  });

  it('accepts the largest representable digest length without TS clamping', async () => {
    await scanner.transform('source', {
      kind: 'pseudonymize',
      key: KEY,
      digestBytes: 0xffff_ffff,
    });

    expect(native.transformPseudonymize.mock.calls[0][4]).toBe(0xffff_ffff);
  });
});

describe('key transport and lifetime', () => {
  it('copies a subarray into a fresh exactly sized buffer', async () => {
    const backing = new Uint8Array(64);
    backing.set(KEY_OTHER, 7);
    const view = backing.subarray(7, 39);
    let passed: ArrayBuffer | null = null;
    let copy: Uint8Array | null = null;
    native.transformPseudonymize.mockImplementation(
      async (_id, _text, buffer: ArrayBuffer) => {
        passed = buffer;
        copy = new Uint8Array(buffer).slice();
        return transformDto();
      },
    );

    await scanner.transform('source', { kind: 'pseudonymize', key: view });

    expect(passed).toBeInstanceOf(ArrayBuffer);
    expect((passed as unknown as ArrayBuffer).byteLength).toBe(32);
    expect([...(copy as unknown as Uint8Array)]).toEqual([...KEY_OTHER]);
    // The caller's backing store is never mutated.
    expect([...backing.subarray(7, 39)]).toEqual([...KEY_OTHER]);
  });

  it('does not mutate the caller key', async () => {
    const key = KEY.slice();
    await scanner.transform('source', { kind: 'pseudonymize', key });
    expect([...key]).toEqual([...KEY]);
  });

  it('zeroes the JavaScript-owned temporary after success', async () => {
    let passed: ArrayBuffer | null = null;
    native.transformSynthesize.mockImplementation(
      async (_id, _text, buffer: ArrayBuffer) => {
        passed = buffer;
        return transformDto();
      },
    );

    await scanner.transform('source', { kind: 'synthesize', key: KEY });

    expect(passed).not.toBeNull();
    expect([...new Uint8Array(passed as unknown as ArrayBuffer)]).toEqual(
      new Array(32).fill(0),
    );
  });

  it('zeroes the JavaScript-owned temporary after a rejection', async () => {
    let passed: ArrayBuffer | null = null;
    native.transformSynthesize.mockImplementation(
      async (_id, _text, buffer: ArrayBuffer) => {
        passed = buffer;
        throw new Error(
          'cribra_native_error:transform_failed:synthesize:source transformation could not be applied safely',
        );
      },
    );

    await expect(
      scanner.transform('source', { kind: 'synthesize', key: KEY }),
    ).rejects.toMatchObject({ code: 'transform_unknown' });

    expect([...new Uint8Array(passed as unknown as ArrayBuffer)]).toEqual(
      new Array(32).fill(0),
    );
    expect([...KEY]).toEqual([...KEY]);
  });

  it.each([
    ['zero bytes', new Uint8Array(0)],
    ['31 bytes', new Uint8Array(31)],
    ['33 bytes', new Uint8Array(33)],
    ['a plain ArrayBuffer', new ArrayBuffer(32)],
    ['an array', Array.from(KEY)],
    ['null', null],
    ['undefined', undefined],
  ])('rejects %s without a native call', async (_name, key) => {
    await expect(
      scanner.transform('source', {
        kind: 'pseudonymize',
        key,
      } as never),
    ).rejects.toMatchObject({ code: 'invalid_key_length' });
    expect(native.transformPseudonymize).not.toHaveBeenCalled();
  });
});

describe('request validation', () => {
  it('rejects an unknown kind before native work', async () => {
    await expect(
      scanner.transform('source', { kind: 'share' } as never),
    ).rejects.toMatchObject({ code: 'invalid_argument' });
    expect(native.transformRedact).not.toHaveBeenCalled();
  });

  it.each([
    ['negative', -1],
    ['fractional', 1.5],
    ['NaN', Number.NaN],
    ['infinite', Number.POSITIVE_INFINITY],
    ['above uint32', 0x1_0000_0000],
    ['string', '16'],
    ['null', null],
  ])('rejects a %s digest length', async (_name, digestBytes) => {
    await expect(
      scanner.transform('source', {
        kind: 'pseudonymize',
        key: KEY,
        digestBytes,
      } as never),
    ).rejects.toMatchObject({ code: 'invalid_argument' });
    expect(native.transformPseudonymize).not.toHaveBeenCalled();
  });

  it('rejects a non-boolean numbered flag', async () => {
    await expect(
      scanner.transform('source', {
        kind: 'template',
        numbered: 'yes',
      } as never),
    ).rejects.toMatchObject({ code: 'invalid_argument' });
    expect(native.transformTemplate).not.toHaveBeenCalled();
  });

  it.each([
    ['source', 'x\uD83D', { kind: 'redact' }],
    ['replacement', 'x', { kind: 'redact', replacement: '\uDC00' }],
    ['template namespace', 'x', { kind: 'template', namespace: 'a\uD83D' }],
    ['pseudonymize prefix', 'x', { kind: 'pseudonymize', key: KEY, prefix: '\uDC00' }],
    ['synthesize marker', 'x', { kind: 'synthesize', key: KEY, marker: 'a\uD83D' }],
  ])('rejects malformed UTF-16 in %s before native work', async (_name, source, request) => {
    await expect(
      scanner.transform(source as string, request as never),
    ).rejects.toMatchObject({ code: 'invalid_unicode_string' });
    expect(native.transformRedact).not.toHaveBeenCalled();
    expect(native.transformTemplate).not.toHaveBeenCalled();
    expect(native.transformPseudonymize).not.toHaveBeenCalled();
    expect(native.transformSynthesize).not.toHaveBeenCalled();
  });

  it('rejects a non-string source', async () => {
    await expect(
      scanner.transform(42 as never, { kind: 'redact' }),
    ).rejects.toMatchObject({ code: 'invalid_argument' });
    expect(native.transformRedact).not.toHaveBeenCalled();
  });
});

describe('fixed C ABI diagnostic classification', () => {
  const diagnostics: readonly [string, string][] = [
    [
      'transform report contains an invalid source span',
      'transform_invalid_span',
    ],
    [
      'transform report span is not aligned to UTF-8 boundaries',
      'transform_invalid_utf8_boundary',
    ],
    [
      'transform requires non-overlapping finding spans',
      'transform_overlapping_spans',
    ],
    [
      'transform source length does not match report',
      'transform_unknown',
    ],
    [
      'source transformation could not be applied safely',
      'transform_unknown',
    ],
  ];

  it.each(diagnostics)('maps %s to %s', async (message, code) => {
    native.transformRedact.mockRejectedValue(
      new Error(`cribra_native_error:transform_failed:redact:${message}`),
    );
    await expect(
      scanner.transform('source', { kind: 'redact' }),
    ).rejects.toMatchObject({ code });
  });

  it('does not classify substring or suffix matches', async () => {
    for (const message of [
      'prefix transform report contains an invalid source span',
      'transform report contains an invalid source span suffix',
      ' transform report contains an invalid source span',
      'TRANSFORM REPORT CONTAINS AN INVALID SOURCE SPAN',
    ]) {
      native.transformTemplate.mockRejectedValueOnce(
        new Error(`cribra_native_error:transform_failed:template:${message}`),
      );
      await expect(
        scanner.transform('source', { kind: 'template' }),
      ).rejects.toMatchObject({ code: 'transform_unknown' });
    }
  });

  it('requires the transform_failed token before classifying', async () => {
    native.transformTemplate.mockRejectedValue(
      new Error(
        'cribra_native_error:invalid_argument:transform report contains an invalid source span',
      ),
    );
    await expect(
      scanner.transform('source', { kind: 'template' }),
    ).rejects.toMatchObject({ code: 'invalid_argument' });
  });

  it('treats a missing diagnostic as unknown', async () => {
    native.transformSynthesize.mockRejectedValue(
      new Error('cribra_native_error:transform_failed:synthesize'),
    );
    await expect(
      scanner.transform('source', { kind: 'synthesize', key: KEY }),
    ).rejects.toMatchObject({ code: 'transform_unknown' });
  });

  it('treats a reworded diagnostic as unknown', async () => {
    native.transformPseudonymize.mockRejectedValue(
      new Error(
        'cribra_native_error:transform_failed:pseudonymize:transform spans overlap',
      ),
    );
    await expect(
      scanner.transform('source', { kind: 'pseudonymize', key: KEY }),
    ).rejects.toMatchObject({ code: 'transform_unknown' });
  });

  it('passes through the key-length native token', async () => {
    native.transformPseudonymize.mockRejectedValue(
      new Error('cribra_native_error:invalid_key_length'),
    );
    await expect(
      scanner.transform('source', { kind: 'pseudonymize', key: KEY }),
    ).rejects.toMatchObject({ code: 'invalid_key_length' });
  });

  it('maps unrelated native failures with the existing policy', async () => {
    native.transformSynthesize.mockRejectedValue(
      new Error('cribra_native_error:scanner_not_found'),
    );
    await expect(
      scanner.transform('source', { kind: 'synthesize', key: KEY }),
    ).rejects.toMatchObject({ code: 'scanner_not_found' });
  });
});

describe('result projection', () => {
  it('projects the full Core-derived scan and output', async () => {
    native.transformRedact.mockResolvedValue(
      transformDto({
        sourceBytes: 9,
        findings: [
          {
            ruleId: 'github.classic-pat',
            start: 1,
            end: 4,
            line: 1,
            column: 2,
            severity: 'critical',
            confidence: 'high',
            remediation: 'revoke_and_rotate_credential',
            explanationKind: 'classified',
            explanationValue: 'deterministic',
          },
        ],
        candidateCount: 1,
        needsReview: true,
        candidates: [
          {
            kind: 'recovery_like_code',
            start: 5,
            end: 9,
            line: 1,
            column: 6,
            evidence: 'structural',
            explanationKind: 'ambiguous',
            explanationValue: 'structural',
          },
        ],
        output: 'redacted',
      }),
    );

    const result = await scanner.transform('source', { kind: 'redact' });

    expect(result.scan).toEqual({
      sourceBytes: 9,
      findings: [
        {
          ruleId: 'github.classic-pat',
          span: { start: 1, end: 4 },
          line: 1,
          column: 2,
          severity: 'critical',
          confidence: 'high',
          remediation: 'revoke_and_rotate_credential',
          explanation: { kind: 'classified', detectionMode: 'deterministic' },
        },
      ],
      candidates: [
        {
          kind: 'recovery_like_code',
          evidence: 'structural',
          span: { start: 5, end: 9 },
          line: 1,
          column: 6,
          explanation: { kind: 'ambiguous', evidence: 'structural' },
        },
      ],
      findingCount: 1,
      candidateCount: 1,
      needsReview: true,
      hasCritical: true,
    });
    expect(result.output).toBe('redacted');
  });

  it('rejects a non-string output before exposing scan fields', async () => {
    native.transformRedact.mockResolvedValue(
      transformDto({ output: 42 as never }),
    );
    await expect(
      scanner.transform('source', { kind: 'redact' }),
    ).rejects.toMatchObject({ code: 'invalid_native_report' });
  });

  it('rejects an invalid projected finding', async () => {
    native.transformRedact.mockResolvedValue(
      transformDto({
        findings: [
          {
            ruleId: 'github.classic-pat',
            start: 4,
            end: 1,
            line: 1,
            column: 1,
            severity: 'critical',
            confidence: 'high',
            remediation: '',
            explanationKind: 'classified',
            explanationValue: 'deterministic',
          },
        ],
        needsReview: true,
      }),
    );
    await expect(
      scanner.transform('source', { kind: 'redact' }),
    ).rejects.toMatchObject({ code: 'invalid_native_report' });
  });
});

describe('disposal precedence', () => {
  it('reports disposed before validating a transform request', async () => {
    await scanner.dispose();
    await expect(
      scanner.transform('source', { kind: 'redact' }),
    ).rejects.toMatchObject({ code: 'disposed' });
    await expect(
      scanner.transform('x\uD83D', { kind: 'pseudonymize', key: new Uint8Array(31) }),
    ).rejects.toMatchObject({ code: 'disposed' });
    expect(native.transformRedact).not.toHaveBeenCalled();
    expect(native.transformPseudonymize).not.toHaveBeenCalled();
  });
});
