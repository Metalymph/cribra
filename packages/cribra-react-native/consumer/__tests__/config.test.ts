/**
 * Facade-level RN.1.3 scanner-configuration tests.
 *
 * The TurboModule is mocked, so these tests pin the JavaScript contract:
 * configuration normalisation, structured build failures, detached rule
 * metadata, strict creation-DTO validation, and cleanup of scanners whose
 * metadata projection fails. Rule metadata and failure cases are compared
 * against a direct-Core oracle generated from the same fixtures.
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
    getLiveScannerCount: jest.fn(() => 0),
  },
}));

import NativeCribraReactNative from '../../src/NativeCribraReactNative';
import {
  CribraError,
  createScanner,
  getLiveScannerCount,
  type CribraErrorCode,
  type RuleMetadata,
  type ScannerConfig,
  type Severity,
} from '../../src/index';
import type {
  RuleMetadataDto,
  ScannerCreationDto,
} from '../../src/NativeCribraReactNative';
import configCasesJson from '../src/fixtures/rn13-config-cases.json';
import oracleJson from '../src/fixtures/rn13-oracle.json';

const native = NativeCribraReactNative as unknown as {
  createScanner: jest.Mock;
  disposeScanner: jest.Mock;
};

interface ConfigRuleCase {
  kind: 'literal' | 'prefix' | 'suffix' | 'pattern';
  id: string;
  value: string;
  severity: string;
  remediation?: string | null;
}

interface ConfigCase {
  name: string;
  current?: boolean;
  financial?: boolean;
  personal?: boolean;
  rules?: ConfigRuleCase[];
  scan?: unknown[];
}

interface OracleRule {
  ruleId: string;
  kind: string;
  detectionMode: string;
  severity: string;
  remediation: string | null;
}

interface OracleCase {
  name: string;
  rules?: OracleRule[];
  error?: { code: number; ruleId: string | null };
}

const configCases = configCasesJson as unknown as ConfigCase[];
const oracleCases = (oracleJson as unknown as { cases: OracleCase[] }).cases;
const oracleByName = new Map(oracleCases.map((entry) => [entry.name, entry]));

const BUILD_ERROR_CODES: readonly CribraErrorCode[] = [
  'invalid_pattern',
  'pattern_matches_empty',
  'missing_capture_group',
  'empty_rule_id',
  'duplicate_rule_id',
  'empty_matcher',
  'too_many_rules',
  'automaton_build',
];

function metadataDto(overrides: Partial<RuleMetadataDto> = {}): RuleMetadataDto {
  return {
    ruleId: 'custom.rule',
    kind: 'literal',
    detectionMode: 'deterministic',
    severity: 'high',
    remediation: 'rotate_credential',
    ...overrides,
  };
}

function creationDto(
  overrides: Partial<ScannerCreationDto> = {},
): ScannerCreationDto {
  return {
    scannerId: 11,
    rules: [metadataDto()],
    failure: '',
    buildCode: 0,
    ruleId: '',
    detail: '',
    ...overrides,
  };
}

function failureDto(
  overrides: Partial<ScannerCreationDto> = {},
): ScannerCreationDto {
  return creationDto({
    scannerId: 0,
    rules: [],
    failure: 'build_error',
    buildCode: 1,
    ruleId: '',
    detail: 'native build diagnostic',
    ...overrides,
  });
}

function configForCase(testCase: ConfigCase): ScannerConfig {
  const rules = (testCase.rules ?? []).map((rule) => {
    const shared = {
      id: rule.id,
      severity: rule.severity as Severity,
      remediation:
        rule.remediation === undefined || rule.remediation === null
          ? undefined
          : (rule.remediation as RuleMetadata['remediation']),
    };
    return rule.kind === 'pattern'
      ? ({ ...shared, kind: 'pattern', pattern: rule.value } as const)
      : ({ ...shared, kind: rule.kind, value: rule.value } as const);
  });

  return {
    builtins: testCase.current === false ? 'none' : 'current',
    financialBuiltins: testCase.financial === true,
    personalBuiltins: testCase.personal === true,
    rules,
  };
}

function expectedNativeParams(testCase: ConfigCase): unknown[] {
  return [
    testCase.current === false ? 'none' : 'current',
    testCase.financial === true,
    testCase.personal === true,
    (testCase.rules ?? []).map((rule) => ({
      kind: rule.kind,
      id: rule.id,
      value: rule.value,
      severity: rule.severity,
      remediation: rule.remediation ?? '',
    })),
  ];
}

function creationFromOracle(testCase: OracleCase, scannerId: number): ScannerCreationDto {
  return creationDto({
    scannerId,
    rules: (testCase.rules ?? []).map((rule) => ({
      ruleId: rule.ruleId,
      kind: rule.kind,
      detectionMode: rule.detectionMode,
      severity: rule.severity,
      remediation: rule.remediation ?? '',
    })),
  });
}

function expectedMetadata(testCase: OracleCase): RuleMetadata[] {
  return (testCase.rules ?? []).map((rule) => ({
    ruleId: rule.ruleId,
    kind: rule.kind as RuleMetadata['kind'],
    detectionMode: rule.detectionMode as RuleMetadata['detectionMode'],
    severity: rule.severity as Severity,
    remediation: rule.remediation as RuleMetadata['remediation'],
  }));
}

function failureFromOracle(testCase: OracleCase): ScannerCreationDto {
  const error = testCase.error;
  if (error === undefined) {
    throw new Error(`oracle case ${testCase.name} has no error`);
  }
  return failureDto({
    failure: error.code <= 3 ? 'rule_error' : 'build_error',
    buildCode: error.code,
    ruleId: error.ruleId ?? '',
    detail: 'native build diagnostic',
  });
}

async function expectCode(
  promise: Promise<unknown>,
  code: CribraErrorCode,
): Promise<CribraError> {
  await expect(promise).rejects.toMatchObject({ name: 'CribraError', code });
  try {
    await promise;
  } catch (error) {
    expect(error).toBeInstanceOf(CribraError);
    return error as CribraError;
  }
  throw new Error('expected a CribraError rejection');
}

beforeEach(() => {
  jest.clearAllMocks();
  native.createScanner.mockResolvedValue(creationDto());
  native.disposeScanner.mockResolvedValue(undefined);
});

describe('configuration normalisation', () => {
  test('omitted configuration and {} both select the canonical catalog', async () => {
    const scanner = await createScanner();
    expect(native.createScanner).toHaveBeenCalledTimes(1);
    expect(native.createScanner).toHaveBeenNthCalledWith(1, 'current', false, false, []);

    await createScanner({});
    expect(native.createScanner).toHaveBeenNthCalledWith(2, 'current', false, false, []);
    expect(scanner.disposed).toBe(false);
  });

  test('an explicit empty scanner disables every catalog', async () => {
    native.createScanner.mockResolvedValue(creationDto({ rules: [] }));
    await createScanner({ builtins: 'none' });
    expect(native.createScanner).toHaveBeenCalledWith('none', false, false, []);
  });

  test('financial and personal catalogs are independent opt-ins', async () => {
    await createScanner({ financialBuiltins: true });
    expect(native.createScanner).toHaveBeenLastCalledWith('current', true, false, []);

    await createScanner({ personalBuiltins: true });
    expect(native.createScanner).toHaveBeenLastCalledWith('current', false, true, []);

    await createScanner({
      builtins: 'none',
      financialBuiltins: true,
      personalBuiltins: true,
    });
    expect(native.createScanner).toHaveBeenLastCalledWith('none', true, true, []);
  });

  test('projects each public rule form into the scalar native DTO', async () => {
    const config = {
      builtins: 'none',
      financialBuiltins: false,
      personalBuiltins: false,
      rules: [
        {
          kind: 'literal',
          id: 'custom.literal',
          value: 'VALUE',
          severity: 'high',
          remediation: 'rotate_credential',
        },
        {
          kind: 'prefix',
          id: 'custom.prefix',
          value: 'pre_',
          severity: 'medium',
        },
        {
          kind: 'suffix',
          id: 'custom.suffix',
          value: '_suf',
          severity: 'low',
          remediation: null,
        },
        {
          kind: 'pattern',
          id: 'custom.pattern',
          pattern: 'PATTERN-[0-9]+',
          severity: 'critical',
          remediation: 'review_sensitive_hash',
        },
      ],
    } satisfies ScannerConfig;

    await createScanner(config);

    expect(native.createScanner).toHaveBeenCalledWith('none', false, false, [
      {
        kind: 'literal',
        id: 'custom.literal',
        value: 'VALUE',
        severity: 'high',
        remediation: 'rotate_credential',
      },
      {
        kind: 'prefix',
        id: 'custom.prefix',
        value: 'pre_',
        severity: 'medium',
        remediation: '',
      },
      {
        kind: 'suffix',
        id: 'custom.suffix',
        value: '_suf',
        severity: 'low',
        remediation: '',
      },
      {
        kind: 'pattern',
        id: 'custom.pattern',
        value: 'PATTERN-[0-9]+',
        severity: 'critical',
        remediation: 'review_sensitive_hash',
      },
    ]);
  });

  test('copies the caller rule array before native work', async () => {
    const rules = [
      {
        kind: 'literal' as const,
        id: 'custom.copy',
        value: 'BEFORE',
        severity: 'high' as const,
      },
    ];
    const config: ScannerConfig = { builtins: 'none', rules };

    await createScanner(config);
    rules[0].value = 'AFTER';

    expect(native.createScanner.mock.calls[0][3]).toEqual([
      {
        kind: 'literal',
        id: 'custom.copy',
        value: 'BEFORE',
        severity: 'high',
        remediation: '',
      },
    ]);
  });

  test.each([
    ['null config', null],
    ['array config', []],
    ['unknown builtins', { builtins: 'all' }],
    ['non-boolean financial flag', { financialBuiltins: 1 }],
    ['non-boolean personal flag', { personalBuiltins: 'yes' }],
    ['non-array rules', { rules: {} }],
    ['null rule', { rules: [null] }],
    ['unknown rule kind', { rules: [{ kind: 'regex', id: 'a', value: 'x', severity: 'high' }] }],
    ['non-string rule id', { rules: [{ kind: 'literal', id: 1, value: 'x', severity: 'high' }] }],
    ['unknown severity', { rules: [{ kind: 'literal', id: 'a', value: 'x', severity: 'severe' }] }],
    [
      'unknown output remediation',
      { rules: [{ kind: 'literal', id: 'a', value: 'x', severity: 'high', remediation: 'unknown' }] },
    ],
    ['pattern without pattern', { rules: [{ kind: 'pattern', id: 'a', severity: 'high' }] }],
    [
      'capture group option',
      { rules: [{ kind: 'pattern', id: 'a', pattern: 'x', severity: 'high', captureGroup: 'v' }] },
    ],
  ] as const)('rejects %s before native work', async (_label, config) => {
    await expectCode(
      createScanner(config as unknown as ScannerConfig),
      'invalid_argument',
    );
    expect(native.createScanner).not.toHaveBeenCalled();
  });
});

describe('detached rule metadata', () => {
  test('exposes frozen metadata that survives disposal', async () => {
    native.createScanner.mockResolvedValue(
      creationDto({
        scannerId: 42,
        rules: [
          metadataDto(),
          metadataDto({
            ruleId: 'github.classic-pat',
            kind: 'prefix',
            detectionMode: 'deterministic',
            severity: 'critical',
            remediation: 'revoke_and_rotate_credential',
          }),
        ],
      }),
    );

    const scanner = await createScanner();
    expect(scanner.rulesCount).toBe(2);
    expect(scanner.rules).toEqual([
      {
        ruleId: 'custom.rule',
        kind: 'literal',
        detectionMode: 'deterministic',
        severity: 'high',
        remediation: 'rotate_credential',
      },
      {
        ruleId: 'github.classic-pat',
        kind: 'prefix',
        detectionMode: 'deterministic',
        severity: 'critical',
        remediation: 'revoke_and_rotate_credential',
      },
    ]);
    expect(Object.isFrozen(scanner.rules)).toBe(true);
    expect(Object.isFrozen(scanner.rules[0])).toBe(true);

    await scanner.dispose();
    expect(scanner.disposed).toBe(true);
    expect(scanner.rulesCount).toBe(2);
    expect(scanner.rules[1]?.ruleId).toBe('github.classic-pat');
  });

  test('maps a null Remediation to empty native text and back to null', async () => {
    native.createScanner.mockResolvedValue(
      creationDto({
        rules: [metadataDto({ remediation: '' })],
      }),
    );

    const scanner = await createScanner();
    expect(scanner.rules[0]?.remediation).toBeNull();
  });
});

describe('direct-Core oracle parity', () => {
  test.each(oracleCases.filter((entry) => entry.error === undefined).map((entry) => entry.name))(
    'metadata matches Core for %s',
    async (name) => {
      const testCase = oracleByName.get(name);
      const configCase = configCases.find((entry) => entry.name === name);
      if (testCase === undefined || configCase === undefined) {
        throw new Error(`missing fixture case ${name}`);
      }
      native.createScanner.mockResolvedValue(creationFromOracle(testCase, 21));

      const scanner = await createScanner(configForCase(configCase));

      expect(native.createScanner).toHaveBeenCalledWith(
        ...expectedNativeParams(configCase),
      );
      expect(scanner.rules).toEqual(expectedMetadata(testCase));
      expect(scanner.rulesCount).toBe((testCase.rules ?? []).length);
    },
  );

  test.each(oracleCases.filter((entry) => entry.error !== undefined).map((entry) => entry.name))(
    'failure matches Core for %s',
    async (name) => {
      const testCase = oracleByName.get(name);
      const configCase = configCases.find((entry) => entry.name === name);
      if (testCase?.error === undefined || configCase === undefined) {
        throw new Error(`missing failure fixture case ${name}`);
      }
      native.createScanner.mockResolvedValue(failureFromOracle(testCase));

      const error = await expectCode(
        createScanner(configForCase(configCase)),
        BUILD_ERROR_CODES[testCase.error.code - 1] as CribraErrorCode,
      );

      expect(error.ruleId).toBe(
        testCase.error.ruleId === null || testCase.error.ruleId === ''
          ? undefined
          : testCase.error.ruleId,
      );
      // Rule identities come from structured metadata only, never messages.
      if (testCase.error.ruleId !== null && testCase.error.ruleId !== '') {
        expect(error.message).not.toContain(testCase.error.ruleId);
        expect(error.detail ?? '').not.toContain(testCase.error.ruleId);
      }
    },
  );
});

describe('structured build failures', () => {
  test.each(BUILD_ERROR_CODES.map((code, index) => [index + 1, code] as const))(
    'maps build code %i to %s',
    async (buildCode, code) => {
      native.createScanner.mockResolvedValue(
        failureDto({ failure: 'build_error', buildCode, ruleId: 'custom.rule' }),
      );
      const error = await expectCode(createScanner(), code);
      expect(error.ruleId).toBe('custom.rule');
      expect(error.message).not.toContain('custom.rule');
      expect(error.detail ?? '').not.toContain('custom.rule');
    },
  );

  test('maps a rule_error category with the same build codes', async () => {
    native.createScanner.mockResolvedValue(
      failureDto({ failure: 'rule_error', buildCode: 2, ruleId: 'custom.pattern' }),
    );
    const error = await expectCode(createScanner(), 'pattern_matches_empty');
    expect(error.ruleId).toBe('custom.pattern');
  });

  test('does not invent a rule id when Core reports none', async () => {
    native.createScanner.mockResolvedValue(failureDto({ buildCode: 4 }));
    const error = await expectCode(createScanner(), 'empty_rule_id');
    expect(error.ruleId).toBeUndefined();
  });

  test('unknown or zero build codes fall back to scanner_create_failed', async () => {
    native.createScanner.mockResolvedValue(
      failureDto({ failure: 'build_error', buildCode: 0, ruleId: 'custom.rule' }),
    );
    await expectCode(createScanner(), 'scanner_create_failed');
    expect(native.disposeScanner).not.toHaveBeenCalled();

    native.createScanner.mockResolvedValue(
      failureDto({ failure: 'build_error', buildCode: 9, ruleId: 'custom.rule' }),
    );
    await expectCode(createScanner(), 'scanner_create_failed');
  });

  test('passes an ABI mismatch through unchanged', async () => {
    native.createScanner.mockResolvedValue(
      failureDto({
        failure: 'abi_mismatch',
        buildCode: 0,
        detail: 'cribra_abi_version:0.3.0:0.2.0',
      }),
    );
    const error = await expectCode(createScanner(), 'abi_mismatch');
    expect(error.detail).toBe('cribra_abi_version:0.3.0:0.2.0');
  });

  test('unknown failure categories become scanner_create_failed', async () => {
    native.createScanner.mockResolvedValue(
      failureDto({ failure: 'mystery_failure', buildCode: 0, detail: 'opaque' }),
    );
    const error = await expectCode(createScanner(), 'scanner_create_failed');
    expect(error.detail).toBe('opaque');
  });
});

describe('strict creation DTO validation and cleanup', () => {
  function malformedMetadata(overrides: Partial<RuleMetadataDto>): ScannerCreationDto {
    return creationDto({ scannerId: 5, rules: [metadataDto(overrides)] });
  }

  test.each([
    ['empty rule id', { ruleId: '' }],
    ['unknown kind', { kind: 'regex' }],
    ['unknown detection mode', { detectionMode: 'none' }],
    ['unknown severity', { severity: 'severe' }],
    ['unknown remediation', { remediation: 'rotate' }],
  ] as const)('rejects malformed metadata (%s) and releases the scanner', async (_label, overrides) => {
    native.createScanner.mockResolvedValue(malformedMetadata(overrides));

    const error = await expectCode(createScanner(), 'invalid_native_report');
    expect(error.detail).toContain('rule.');
    expect(native.disposeScanner).toHaveBeenCalledTimes(1);
    expect(native.disposeScanner).toHaveBeenCalledWith(5);
    expect(getLiveScannerCount()).toBe(0);
  });

  test('rejects a success DTO with scannerId 0 without dispatching cleanup', async () => {
    native.createScanner.mockResolvedValue(
      creationDto({ scannerId: 0, rules: [] }),
    );
    const error = await expectCode(createScanner(), 'invalid_native_report');
    expect(error.detail).toContain('creation.scannerId');
    expect(native.disposeScanner).not.toHaveBeenCalled();
  });

  test('rejects an inconsistent success DTO', async () => {
    native.createScanner.mockResolvedValue(
      creationDto({ scannerId: 5, buildCode: 1 }),
    );
    await expectCode(createScanner(), 'invalid_native_report');
    expect(native.disposeScanner).toHaveBeenCalledWith(5);

    native.createScanner.mockResolvedValue(
      creationDto({ scannerId: 5, ruleId: 'custom.rule' }),
    );
    await expectCode(createScanner(), 'invalid_native_report');
  });

  test('rejects a failure DTO that still carries a scanner id, releasing it', async () => {
    native.createScanner.mockResolvedValue(
      failureDto({ scannerId: 8, buildCode: 1 }),
    );
    await expectCode(createScanner(), 'invalid_native_report');
    expect(native.disposeScanner).toHaveBeenCalledTimes(1);
    expect(native.disposeScanner).toHaveBeenCalledWith(8);
  });

  test('rejects malformed field types before projecting', async () => {
    const cases: ScannerCreationDto[] = [
      creationDto({ failure: 1 as unknown as string }),
      creationDto({ ruleId: 1 as unknown as string }),
      creationDto({ detail: 1 as unknown as string }),
      creationDto({ buildCode: -1 }),
      creationDto({ buildCode: 1.5 }),
      creationDto({ buildCode: Number.MAX_SAFE_INTEGER + 1 }),
      creationDto({ rules: 'nope' as unknown as RuleMetadataDto[] }),
    ];
    for (const dto of cases) {
      native.createScanner.mockResolvedValue(dto);
      await expectCode(createScanner(), 'invalid_native_report');
    }
  });

  test('rejects an explicitly invalid_native_report failure category', async () => {
    native.createScanner.mockResolvedValue(
      failureDto({ failure: 'invalid_native_report', buildCode: 0 }),
    );
    const error = await expectCode(createScanner(), 'invalid_native_report');
    expect(error.detail).toContain('creation');
  });

  test('cleanup failure never masks the original projection error', async () => {
    native.createScanner.mockResolvedValue(
      malformedMetadata({ kind: 'regex' }),
    );
    native.disposeScanner.mockRejectedValue(new Error('cleanup boom'));

    const error = await expectCode(createScanner(), 'invalid_native_report');
    expect(error.detail).toContain('rule.kind');
    expect(native.disposeScanner).toHaveBeenCalledWith(5);
  });

  test('a failed scanner construction never registers a scanner', async () => {
    native.createScanner.mockResolvedValue(
      failureDto({ buildCode: 6, ruleId: 'custom.rule' }),
    );
    await expectCode(createScanner(), 'empty_matcher');
    expect(native.disposeScanner).not.toHaveBeenCalled();
  });
});

describe('native rejection normalisation', () => {
  test('preserves an ABI mismatch raised as a rejection', async () => {
    native.createScanner.mockRejectedValue(
      new Error('cribra_native_error:abi_mismatch:0.3.0:0.2.0'),
    );
    const error = await expectCode(createScanner(), 'abi_mismatch');
    expect(error.detail).toBe('cribra_native_error:abi_mismatch:0.3.0:0.2.0');
  });

  test('folds other native rejections into scanner_create_failed', async () => {
    native.createScanner.mockRejectedValue(
      new Error('cribra_native_error:scanner_id_exhausted'),
    );
    const error = await expectCode(createScanner(), 'scanner_create_failed');
    expect(error.detail).toBe('cribra_native_error:scanner_id_exhausted');

    native.createScanner.mockRejectedValue(new Error('boom'));
    await expectCode(createScanner(), 'scanner_create_failed');
  });
});
