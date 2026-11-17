/**
 * RN.1.4 preflight tests.
 *
 * The public scan paths must reject malformed UTF-16 before any native work is
 * dispatched. Hermes would silently substitute U+FFFD and JSC would silently
 * truncate, so a native call for such a source would produce offsets for a
 * different string.
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
import { utf8ByteLength } from '../../src/offset';
import type {
  ScanBatchDto,
  ScanReportDto,
  ScannerCreationDto,
} from '../../src/NativeCribraReactNative';

const native = NativeCribraReactNative as unknown as {
  createScanner: jest.Mock;
  scan: jest.Mock;
  scanBatch: jest.Mock;
};

function creationDto(): ScannerCreationDto {
  return {
    scannerId: 11,
    rules: [],
    failure: '',
    buildCode: 0,
    ruleId: '',
    detail: '',
  };
}

function report(sourceId: string, sourceBytes: number): ScanReportDto {
  return {
    sourceId,
    sourceBytes,
    findings: [],
    candidateCount: 0,
    needsReview: false,
    candidates: [],
  };
}

function batch(entries: ScanReportDto[]): ScanBatchDto {
  return {
    entries,
    summary: {
      scannedSources: entries.length,
      scannedBytes: entries.reduce((sum, entry) => sum + entry.sourceBytes, 0),
      reportsWithFindings: 0,
      reportsWithoutFindings: entries.length,
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
    },
  };
}

let scanner: CribraScanner;

beforeEach(async () => {
  native.createScanner.mockResolvedValue(creationDto());
  native.scan.mockReset();
  native.scanBatch.mockReset();
  native.scan.mockImplementation(async (_id, sourceId: string, text: string) =>
    report(sourceId, utf8ByteLength(text)),
  );
  native.scanBatch.mockImplementation(
    async (_id, sources: { id: string; source: string }[]) =>
      batch(
        sources.map((source) =>
          report(source.id, utf8ByteLength(source.source)),
        ),
      ),
  );
  scanner = await createScanner();
});

const MALFORMED = [
  ['lone high at start', '\uD83Dabc'],
  ['lone high in the middle', 'ab\uD83Dcd'],
  ['lone high at the end', 'abc\uD83D'],
  ['lone low at start', '\uDC00abc'],
  ['lone low in the middle', 'ab\uDC00cd'],
  ['lone low at the end', 'abc\uDC00'],
] as const;

describe('scan preflight', () => {
  it.each(MALFORMED)('rejects %s without a native call', async (_name, text) => {
    await expect(scanner.scan(text)).rejects.toMatchObject({
      name: 'CribraError',
      code: 'invalid_unicode_string',
    });
    expect(native.scan).not.toHaveBeenCalled();
  });

  it('rejects before the native call even with a valid id', async () => {
    await expect(
      scanner.scan('x\uD83D', { id: 'doc' }),
    ).rejects.toMatchObject({ code: 'invalid_unicode_string' });
    expect(native.scan).not.toHaveBeenCalled();
  });

  it('accepts well-formed supplementary characters', async () => {
    const result = await scanner.scan('a🙂b');
    expect(native.scan).toHaveBeenCalledTimes(1);
    expect(result.sourceBytes).toBe(6);
  });

  it('accepts empty and ASCII sources', async () => {
    await scanner.scan('');
    await scanner.scan('plain ASCII');
    expect(native.scan).toHaveBeenCalledTimes(2);
  });
});

describe('scanBatch preflight', () => {
  it.each(MALFORMED)(
    'rejects a batch with %s at position 0 without a native call',
    async (_name, text) => {
      await expect(
        scanner.scanBatch([
          { id: 'bad', source: text },
          { id: 'good', source: 'plain' },
        ]),
      ).rejects.toMatchObject({ code: 'invalid_unicode_string' });
      expect(native.scanBatch).not.toHaveBeenCalled();
      expect(native.scan).not.toHaveBeenCalled();
    },
  );

  it('rejects when the malformed source is last', async () => {
    await expect(
      scanner.scanBatch([
        { id: 'a', source: 'plain' },
        { id: 'b', source: 'é🙂' },
        { id: 'c', source: '\uDC00' },
      ]),
    ).rejects.toMatchObject({ code: 'invalid_unicode_string' });
    expect(native.scanBatch).not.toHaveBeenCalled();
  });

  it('reports the first malformed source deterministically', async () => {
    await expect(
      scanner.scanBatch([
        { id: 'a', source: '\uD83D' },
        { id: 'b', source: '\uDC00' },
      ]),
    ).rejects.toMatchObject({
      message: 'A Cribra string contains an unpaired UTF-16 surrogate.',
    });
    expect(native.scanBatch).not.toHaveBeenCalled();
  });

  it('accepts a fully well-formed batch', async () => {
    const result = await scanner.scanBatch([
      { id: 'a', source: 'plain' },
      { id: 'b', source: 'é🙂e\u0301' },
      { id: 'c', source: '' },
    ]);
    expect(native.scanBatch).toHaveBeenCalledTimes(1);
    expect(result.entries).toHaveLength(3);
    expect(result.entries[1].sourceBytes).toBe(utf8ByteLength('é🙂e\u0301'));
  });

  it('keeps structural validation ahead of dispatch', async () => {
    await expect(
      scanner.scanBatch([{ id: 1, source: 'plain' } as never]),
    ).rejects.toMatchObject({ code: 'invalid_argument' });
    expect(native.scanBatch).not.toHaveBeenCalled();
  });
});

describe('disposal precedence', () => {
  it('reports disposed before validating a malformed source', async () => {
    await scanner.dispose();
    await expect(scanner.scan('a\uD83Db')).rejects.toMatchObject({
      code: 'disposed',
    });
    await expect(
      scanner.scanBatch([{ id: 'x', source: '\uDC00' }]),
    ).rejects.toMatchObject({ code: 'disposed' });
    expect(native.scan).not.toHaveBeenCalled();
    expect(native.scanBatch).not.toHaveBeenCalled();
  });
});
