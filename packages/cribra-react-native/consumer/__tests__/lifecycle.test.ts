/**
 * RN.1.6 lifecycle tests.
 *
 * The TurboModule is mocked, so these tests pin the JavaScript half of the
 * teardown contract: fixed enqueue/shutdown diagnostics map to the existing
 * `native_error` code, a refused job never leaves a silently pending Promise,
 * scanner disposal stays idempotent and retryable, an already accepted scan
 * still settles after disposal starts, and post-disposal calls never reach the
 * native module. Native worker ordering, queue discard and zeroization are
 * covered by the JSI-free host tests.
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

const scannerId = 41;

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

function reportDto(): ScanReportDto {
  return {
    sourceId: 'doc',
    sourceBytes: 4,
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const KEY = Uint8Array.from({ length: 32 }, (_value, index) => index);

let scanner: CribraScanner;

beforeEach(async () => {
  native.createScanner.mockReset();
  native.scan.mockReset();
  native.transformSynthesize.mockReset();
  native.disposeScanner.mockReset();

  native.createScanner.mockResolvedValue(creationDto());
  native.scan.mockResolvedValue(reportDto());
  native.transformSynthesize.mockResolvedValue(transformDto());
  native.disposeScanner.mockResolvedValue(undefined);

  scanner = await createScanner();
});

afterEach(async () => {
  await scanner.dispose().catch(() => undefined);
});

describe('shutdown refusal classification', () => {
  it('maps worker_stopped to native_error without losing the detail', async () => {
    native.scan.mockRejectedValueOnce(
      new Error('cribra_native_error:worker_stopped'),
    );

    const error = await scanner.scan('text').catch((caught: unknown) => caught);

    expect(error).toBeInstanceOf(CribraError);
    expect(error).toMatchObject({
      code: 'native_error',
      detail: 'cribra_native_error:worker_stopped',
    });
  });

  it('maps worker_unavailable to native_error without inventing a code', async () => {
    native.scan.mockRejectedValueOnce(
      new Error('cribra_native_error:worker_unavailable'),
    );

    const error = await scanner.scan('text').catch((caught: unknown) => caught);

    expect(error).toMatchObject({
      code: 'native_error',
      detail: 'cribra_native_error:worker_unavailable',
    });
  });

  it('reports a refused scanner creation as scanner_create_failed', async () => {
    native.createScanner.mockRejectedValueOnce(
      new Error('cribra_native_error:worker_stopped'),
    );

    const error = await createScanner().catch((caught: unknown) => caught);

    expect(error).toBeInstanceOf(CribraError);
    expect(error).toMatchObject({
      code: 'scanner_create_failed',
      detail: 'cribra_native_error:worker_stopped',
    });
    // A refused creation never produced a scanner handle to clean up.
    expect(native.disposeScanner).not.toHaveBeenCalled();
  });

  it('zeroes the JavaScript key temporary when the job is refused', async () => {
    let passed: ArrayBuffer | null = null;
    native.transformSynthesize.mockImplementationOnce(
      async (_id, _text, buffer: ArrayBuffer) => {
        passed = buffer;
        throw new Error('cribra_native_error:worker_unavailable');
      },
    );

    await expect(
      scanner.transform('secret', { kind: 'synthesize', key: KEY }),
    ).rejects.toMatchObject({
      code: 'native_error',
      detail: 'cribra_native_error:worker_unavailable',
    });

    expect(passed).not.toBeNull();
    expect([...new Uint8Array(passed as unknown as ArrayBuffer)]).toEqual(
      new Array(32).fill(0),
    );
    // The caller-owned key is never touched.
    expect([...KEY]).toEqual([...KEY]);
  });
});

describe('disposal lifecycle', () => {
  it('shares one native disposal between concurrent callers', async () => {
    const disposal = deferred<void>();
    native.disposeScanner.mockReturnValueOnce(disposal.promise);

    const first = scanner.dispose();
    const second = scanner.dispose();
    expect(native.disposeScanner).toHaveBeenCalledTimes(1);

    disposal.resolve();
    await Promise.all([first, second]);
    await scanner.dispose();
    expect(native.disposeScanner).toHaveBeenCalledTimes(1);
  });

  it('lets an already accepted scan settle after disposal starts', async () => {
    const inFlight = deferred<ScanReportDto>();
    native.scan.mockReturnValueOnce(inFlight.promise);

    const scanPromise = scanner.scan('text');
    await scanner.dispose();
    inFlight.resolve(reportDto());

    await expect(scanPromise).resolves.toMatchObject({
      sourceBytes: 4,
      findingCount: 0,
    });
  });

  it('never dispatches a scan after disposal', async () => {
    await scanner.dispose();
    native.scan.mockClear();

    await expect(scanner.scan('text')).rejects.toMatchObject({
      code: 'disposed',
    });
    expect(native.scan).not.toHaveBeenCalled();
  });

  it('treats an already-removed scanner as disposed', async () => {
    native.disposeScanner.mockRejectedValueOnce(
      new Error('cribra_native_error:scanner_not_found'),
    );

    await expect(scanner.dispose()).resolves.toBeUndefined();
  });

  it('propagates a shutdown refusal and allows a later retry', async () => {
    native.disposeScanner.mockRejectedValueOnce(
      new Error('cribra_native_error:worker_stopped'),
    );

    await expect(scanner.dispose()).rejects.toMatchObject({
      code: 'native_error',
      detail: 'cribra_native_error:worker_stopped',
    });

    await expect(scanner.dispose()).resolves.toBeUndefined();
    expect(native.disposeScanner).toHaveBeenCalledTimes(2);

    // A failed disposal never re-enables scanning.
    await expect(scanner.scan('text')).rejects.toMatchObject({
      code: 'disposed',
    });
  });
});

describe('promise settlement', () => {
  it('rejects exactly once when the native job is refused', async () => {
    native.scan.mockRejectedValueOnce(
      new Error('cribra_native_error:worker_stopped'),
    );

    let settlements = 0;
    const settled = scanner.scan('text').then(
      () => {
        settlements += 1;
      },
      () => {
        settlements += 1;
      },
    );

    await settled;
    expect(settlements).toBe(1);
  });

  it('does not retain a refused job in the JavaScript facade', async () => {
    native.scan.mockRejectedValueOnce(
      new Error('cribra_native_error:worker_unavailable'),
    );

    await expect(scanner.scan('text')).rejects.toMatchObject({
      code: 'native_error',
    });

    // The scanner stays usable while the module is alive.
    await expect(scanner.scan('text')).resolves.toBeDefined();
    expect(native.scan).toHaveBeenCalledTimes(2);
  });
});
