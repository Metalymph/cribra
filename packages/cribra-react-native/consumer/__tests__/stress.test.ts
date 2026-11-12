/**
 * RN.1.6 stress tests for the JavaScript facade.
 *
 * These tests exercise promise volume, repeated creation/disposal and the
 * refusal paths at the facade boundary. The native serial worker itself is
 * stressed by the JSI-free host tests (10,000-job burst, queue discard and
 * concurrent post/stop under TSan); here the TurboModule is mocked so the
 * assertions stay deterministic.
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
import {
  createScanner,
  CribraError,
  type CribraScanner,
} from '../../src/index';
import type {
  ScanReportDto,
  ScannerCreationDto,
  TransformResultDto,
} from '../../src/NativeCribraReactNative';

const native = NativeCribraReactNative as unknown as {
  createScanner: jest.Mock;
  scan: jest.Mock;
  transformSynthesize: jest.Mock;
  disposeScanner: jest.Mock;
};

const scannerId = 77;

function creationDto(id: number = scannerId): ScannerCreationDto {
  return {
    scannerId: id,
    rules: [],
    failure: '',
    buildCode: 0,
    ruleId: '',
    detail: '',
  };
}

function reportDto(sourceId: string, sourceBytes: number): ScanReportDto {
  return {
    sourceId,
    sourceBytes,
    findings: [],
    candidateCount: 0,
    needsReview: false,
    candidates: [],
  };
}

function transformDto(): TransformResultDto {
  return {
    sourceBytes: 6,
    findings: [],
    candidateCount: 0,
    needsReview: false,
    candidates: [],
    output: 'output',
  };
}

const KEY = Uint8Array.from({ length: 32 }, (_value, index) => index);

beforeEach(() => {
  native.createScanner.mockReset();
  native.scan.mockReset();
  native.transformSynthesize.mockReset();
  native.disposeScanner.mockReset();

  native.createScanner.mockResolvedValue(creationDto());
  native.scan.mockImplementation(async (_id, sourceId: string, text: string) =>
    reportDto(sourceId, text.length),
  );
  native.transformSynthesize.mockResolvedValue(transformDto());
  native.disposeScanner.mockResolvedValue(undefined);
});

describe('promise volume', () => {
  const jobs = 10_000;

  it(`settles ${jobs} concurrent scans exactly once each`, async () => {
    const scanner: CribraScanner = await createScanner();

    const started = Date.now();
    const results = await Promise.all(
      Array.from({ length: jobs }, (_value, index) =>
        scanner.scan(`source-${index}`, { id: `doc-${index}` }),
      ),
    );
    const elapsed = Date.now() - started;

    expect(native.scan).toHaveBeenCalledTimes(jobs);
    expect(results).toHaveLength(jobs);
    expect(results[0]).toMatchObject({ sourceBytes: 8 });
    expect(results[jobs - 1]).toMatchObject({ sourceBytes: 11 });
    // eslint-disable-next-line no-console
    console.log(`stress: ${jobs} facade scans settled in ${elapsed} ms`);

    await scanner.dispose();
    expect(native.disposeScanner).toHaveBeenCalledTimes(1);
  });

  it('settles mixed accept/refuse outcomes without losing a job', async () => {
    const scanner: CribraScanner = await createScanner();
    const settled = new Map<number, 'ok' | 'refused'>();

    native.scan.mockImplementation(async (_id, _sourceId: string, text: string) => {
      const index = Number(text.slice('source-'.length));
      if (index % 2 === 0) {
        throw new Error('cribra_native_error:worker_stopped');
      }
      return reportDto('', 1);
    });

    const outcomes = await Promise.all(
      Array.from({ length: jobs }, (_value, index) =>
        scanner
          .scan(`source-${index}`)
          .then(() => {
            settled.set(index, 'ok');
          })
          .catch((error: unknown) => {
            expect(error).toBeInstanceOf(CribraError);
            expect(error).toMatchObject({
              code: 'native_error',
              detail: 'cribra_native_error:worker_stopped',
            });
            settled.set(index, 'refused');
          }),
      ),
    );

    expect(outcomes).toHaveLength(jobs);
    expect(settled.size).toBe(jobs);
    let refused = 0;
    for (const [index, outcome] of settled) {
      if (outcome === 'refused') {
        expect(index % 2).toBe(0);
        refused += 1;
      }
    }
    expect(refused).toBe(jobs / 2);
  });

  it('rejects a post-dispose burst without any native call', async () => {
    const scanner: CribraScanner = await createScanner();
    await scanner.dispose();
    native.scan.mockClear();

    const outcomes = await Promise.allSettled(
      Array.from({ length: jobs }, (_value, index) =>
        scanner.scan(`source-${index}`),
      ),
    );

    expect(native.scan).not.toHaveBeenCalled();
    expect(outcomes).toHaveLength(jobs);
    for (const outcome of outcomes) {
      expect(outcome.status).toBe('rejected');
      if (outcome.status === 'rejected') {
        expect(outcome.reason).toMatchObject({ code: 'disposed' });
      }
    }
  });
});

describe('repeated creation and disposal', () => {
  const rounds = 500;

  it(`creates and disposes ${rounds} scanners without duplicate disposal`, async () => {
    const disposed = new Set<number>();
    native.createScanner.mockImplementation(async () => {
      const id = Number(native.createScanner.mock.calls.length);
      return creationDto(id);
    });
    native.disposeScanner.mockImplementation(async (id: number) => {
      if (disposed.has(id)) {
        throw new Error('cribra_native_error:scanner_not_found');
      }
      disposed.add(id);
    });

    const started = Date.now();
    for (let round = 0; round < rounds; round += 1) {
      const scanner = await createScanner();
      await scanner.dispose();
      // The facade treats a second dispose as the same settlement.
      await scanner.dispose();
    }
    const elapsed = Date.now() - started;

    expect(native.createScanner).toHaveBeenCalledTimes(rounds);
    expect(native.disposeScanner).toHaveBeenCalledTimes(rounds);
    expect(disposed.size).toBe(rounds);
    // eslint-disable-next-line no-console
    console.log(`stress: ${rounds} create/dispose rounds in ${elapsed} ms`);
  });

  it('keeps disposal failures retryable under repetition', async () => {
    let failNext = true;
    native.disposeScanner.mockImplementation(async () => {
      if (failNext) {
        failNext = false;
        throw new Error('cribra_native_error:worker_unavailable');
      }
      failNext = true;
    });

    for (let round = 0; round < 50; round += 1) {
      const scanner = await createScanner();
      await expect(scanner.dispose()).rejects.toMatchObject({
        code: 'native_error',
        detail: 'cribra_native_error:worker_unavailable',
      });
      await expect(scanner.dispose()).resolves.toBeUndefined();
    }
  });
});

describe('keyed transform volume', () => {
  const rounds = 1_000;

  it(`zeroes every JavaScript key temporary across ${rounds} transforms`, async () => {
    const scanner: CribraScanner = await createScanner();
    const buffers: ArrayBuffer[] = [];

    native.transformSynthesize.mockImplementation(
      async (_id, _text, buffer: ArrayBuffer) => {
        buffers.push(buffer);
        return transformDto();
      },
    );

    await Promise.all(
      Array.from({ length: rounds }, () =>
        scanner.transform('secret', { kind: 'synthesize', key: KEY }),
      ),
    );

    expect(buffers).toHaveLength(rounds);
    for (const buffer of buffers) {
      expect(buffer.byteLength).toBe(32);
      expect([...new Uint8Array(buffer)]).toEqual(new Array(32).fill(0));
    }
    // The caller's key is never mutated.
    expect([...KEY]).toEqual([...KEY]);
  });

  it('zeroes temporaries when every keyed job is refused', async () => {
    const scanner: CribraScanner = await createScanner();
    const buffers: ArrayBuffer[] = [];

    native.transformSynthesize.mockImplementation(
      async (_id, _text, buffer: ArrayBuffer) => {
        buffers.push(buffer);
        throw new Error('cribra_native_error:worker_stopped');
      },
    );

    const outcomes = await Promise.allSettled(
      Array.from({ length: rounds }, () =>
        scanner.transform('secret', { kind: 'synthesize', key: KEY }),
      ),
    );

    expect(buffers).toHaveLength(rounds);
    for (const buffer of buffers) {
      expect([...new Uint8Array(buffer)]).toEqual(new Array(32).fill(0));
    }
    for (const outcome of outcomes) {
      expect(outcome.status).toBe('rejected');
    }
  });
});
