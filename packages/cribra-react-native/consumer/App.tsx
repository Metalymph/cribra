/**
 * Cribra RN.1.6 isolated consumer harness.
 *
 * This screen runs the synthetic-fixture parity check, the atomic batch
 * behaviours, the scanner lifecycle checks, the configured-scanner and detached
 * metadata checks against the direct-Core oracle, the exact UTF-8 byte <->
 * UTF-16 span correspondence checks, the malformed-source preflight checks,
 * deterministic native negative cases, the lifecycle stress/recovery phases,
 * and the Flow transformation checks against the native TurboModule. It
 * intentionally displays and logs only aggregate counts and pass/fail markers;
 * no source text, matcher payload, rule ID, or individual finding metadata is
 * logged.
 */

import { useEffect, useState } from 'react';
import {
  DevSettings,
  Pressable,
  StatusBar,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import {
  CribraError,
  byteSpanToUtf16Range,
  byteSpansToUtf16Ranges,
  createScanner,
  getAbiVersion,
  getLiveScannerCount,
  utf16RangeToByteSpan,
  type BatchResult,
  type RuleMetadata,
  type ScanSource,
  type ScannerConfig,
  type Severity,
  type TransformRequest,
} from '@metalymph/cribra-react-native';
// Direct TurboModule access is used only by the deterministic negative cases
// below (invalid scanner id and malformed arguments) and by the numeric-only
// native encoding probe. The facade remains the public API under test
// everywhere else.
import NativeCribraReactNative from '../src/NativeCribraReactNative';
import { assertWellFormedUtf16 } from '../src/offset';

const fixtureSources = require('./src/fixtures/synthetic-source.json') as {
  id: string;
  text: string;
}[];
const sources: ScanSource[] = fixtureSources.map(({ id, text }) => ({
  id,
  source: text,
}));
const oracle = require('./src/fixtures/synthetic-oracle.json') as {
  abiVersion: { major: number; minor: number; patch: number };
  entries: unknown;
  summary: unknown;
};

const configCases = require('./src/fixtures/rn13-config-cases.json') as ConfigCase[];
const configOracle = require('./src/fixtures/rn13-oracle.json') as {
  cases: OracleConfigCase[];
};

const transformCases = require('./src/fixtures/rn15-transform-cases.json') as TransformCase[];
const transformOracle = require('./src/fixtures/rn15-transform-oracle.json') as {
  cases: OracleTransformCase[];
};

interface ConfigCaseRule {
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
  rules?: ConfigCaseRule[];
  scan?: { id: string; text: string }[];
}

interface OracleRule {
  ruleId: string;
  kind: string;
  detectionMode: string;
  severity: string;
  remediation: string | null;
}

interface OracleConfigCase {
  name: string;
  rules?: OracleRule[];
  entries?: unknown;
  summary?: unknown;
  error?: { code: number; ruleId: string | null };
}

interface TransformCase {
  name: string;
  current?: boolean;
  financial?: boolean;
  personal?: boolean;
  rules?: ConfigCaseRule[];
  source: string;
  key?: number[];
  request: {
    kind: 'redact' | 'template' | 'pseudonymize' | 'synthesize';
    replacement?: string;
    namespace?: string;
    numbered?: boolean;
    prefix?: string;
    digestBytes?: number;
    marker?: string;
  };
}

interface OracleTransformCase {
  name: string;
  scan?: unknown;
  output?: string;
  error?: string;
}

const configCaseByName = new Map(configCases.map((entry) => [entry.name, entry]));
const configOracleByName = new Map(
  configOracle.cases.map((entry) => [entry.name, entry]),
);
const transformCaseByName = new Map(
  transformCases.map((entry) => [entry.name, entry]),
);
const transformOracleByName = new Map(
  transformOracle.cases.map((entry) => [entry.name, entry]),
);

const BUILD_ERROR_LABELS = [
  'invalid_pattern',
  'pattern_matches_empty',
  'missing_capture_group',
  'empty_rule_id',
  'duplicate_rule_id',
  'empty_matcher',
  'too_many_rules',
  'automaton_build',
] as const;

function scannerConfigFor(testCase: ConfigCase): ScannerConfig {
  return {
    builtins: testCase.current === false ? 'none' : 'current',
    financialBuiltins: testCase.financial === true,
    personalBuiltins: testCase.personal === true,
    rules: (testCase.rules ?? []).map((rule) => {
      const remediation =
        rule.remediation === undefined || rule.remediation === null
          ? undefined
          : (rule.remediation as NonNullable<RuleMetadata['remediation']>);
      return rule.kind === 'pattern'
        ? {
            kind: 'pattern' as const,
            id: rule.id,
            pattern: rule.value,
            severity: rule.severity as Severity,
            remediation,
          }
        : {
            kind: rule.kind,
            id: rule.id,
            value: rule.value,
            severity: rule.severity as Severity,
            remediation,
          };
    }),
  };
}

function oracleMetadata(testCase: OracleConfigCase): RuleMetadata[] {
  return (testCase.rules ?? []).map((rule) => ({
    ruleId: rule.ruleId,
    kind: rule.kind as RuleMetadata['kind'],
    detectionMode: rule.detectionMode as RuleMetadata['detectionMode'],
    severity: rule.severity as Severity,
    remediation: rule.remediation as RuleMetadata['remediation'],
  }));
}

// Tokens and candidate-shaped values that must never appear in emitted logs.
const secretFragments = [
  'ghp_AbCdEf0123456789_AbCdEf0123456789',
  'sk_test_AbCdEf0123456789_AbCdEf0123456789',
  'ABCD-EFGH-IJKL-MNOP',
  // Custom-rule matcher payloads must never reach emitted logs or the marker.
  'LITERAL_SECRET',
  'PATTERN-42',
  'ACME_CATALOG_TOKEN',
  'ACME-1234-XY',
  'acme_pat_',
  'FIRST_VALUE',
  'SECOND_VALUE',
];

type Outcome =
  | { state: 'running' }
  | {
      state: 'pass';
      abi: string;
      entries: number;
      findings: number;
      candidates: number;
      /** Numeric-only native UTF-8 length for the malformed-string probe. */
      probe: string;
    }
  | { state: 'fail'; reason: string };

/**
 * Test-only structural diff. Reports only the path of the first difference,
 * never values, so the smoke output stays source-free.
 */
function diffJson(a: unknown, b: unknown, path = '$'): string | null {
  if (a === b) {
    return null;
  }
  if (typeof a !== typeof b) {
    return `${path}#type`;
  }
  if (a === null || b === null || typeof a !== 'object') {
    return `${path}#value`;
  }
  const aArray = Array.isArray(a);
  const bArray = Array.isArray(b);
  if (aArray !== bArray) {
    return `${path}#shape`;
  }
  if (aArray && bArray) {
    if (a.length !== b.length) {
      return `${path}#length`;
    }
    for (let index = 0; index < a.length; index += 1) {
      const diff = diffJson(a[index], b[index], `${path}[${index}]`);
      if (diff !== null) {
        return diff;
      }
    }
    return null;
  }
  const aObject = a as Record<string, unknown>;
  const bObject = b as Record<string, unknown>;
  const aKeys = Object.keys(aObject).sort();
  const bKeys = Object.keys(bObject).sort();
  if (aKeys.length !== bKeys.length) {
    return `${path}#keys`;
  }
  for (let index = 0; index < aKeys.length; index += 1) {
    if (aKeys[index] !== bKeys[index]) {
      return `${path}#keys`;
    }
  }
  for (const key of aKeys) {
    const diff = diffJson(aObject[key], bObject[key], `${path}.${key}`);
    if (diff !== null) {
      return diff;
    }
  }
  return null;
}

/**
 * Independent UTF-8 byte length, so source lengths are checked without trusting
 * either the native result or the oracle. Handles surrogate pairs.
 */
function utf8ByteLength(value: string): number {
  let bytes = 0;
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code < 0x80) {
      bytes += 1;
    } else if (code < 0x800) {
      bytes += 2;
    } else if (
      code >= 0xd800 &&
      code <= 0xdbff &&
      index + 1 < value.length &&
      value.charCodeAt(index + 1) >= 0xdc00 &&
      value.charCodeAt(index + 1) <= 0xdfff
    ) {
      bytes += 4;
      index += 1;
    } else {
      bytes += 3;
    }
  }
  return bytes;
}

function marker(outcome: Outcome): string | undefined {
  if (outcome.state === 'pass') {
    return [
      'CRIBRA_RN_SMOKE:PASS',
      `abi=${outcome.abi}`,
      `entries=${outcome.entries}`,
      `findings=${outcome.findings}`,
      `candidates=${outcome.candidates}`,
      'lifecycle=ok',
      'parity=ok',
      'batch=ok',
      'config=ok',
      'transform=ok',
      'key=ok',
      'negative=ok',
      'offset=ok',
      `probe=${outcome.probe}`,
      'privacy=ok',
      'stress=ok',
      'recovery=ok',
    ].join(':');
  }
  if (outcome.state === 'fail') {
    return `CRIBRA_RN_SMOKE:FAIL:${outcome.reason}`;
  }
  return undefined;
}

/** Returns true only for the expected structured error code. */
function isCribraError(error: unknown, code: string): boolean {
  return error instanceof CribraError && error.code === code;
}

/** Returns true only for the expected source-free native error prefix. */
function isNativePrefix(error: unknown, prefix: string): boolean {
  return error instanceof Error && error.message.startsWith(prefix);
}

function isZeroSummary(batch: BatchResult): boolean {
  const summary = batch.summary;
  return (
    batch.entries.length === 0 &&
    summary.scannedSources === 0 &&
    summary.scannedBytes === 0 &&
    summary.reportsWithFindings === 0 &&
    summary.reportsWithoutFindings === 0 &&
    summary.totalFindings === 0 &&
    summary.totalCandidates === 0 &&
    summary.reportsWithCandidates === 0 &&
    !summary.hasCandidates &&
    summary.critical === 0 &&
    summary.high === 0 &&
    summary.medium === 0 &&
    summary.low === 0 &&
    summary.info === 0 &&
    summary.isClean &&
    !summary.hasCritical
  );
}

/** True when `index` falls between the two halves of a surrogate pair. */
function splitsSurrogatePair(text: string, index: number): boolean {
  if (index <= 0 || index >= text.length) {
    return false;
  }
  const previous = text.charCodeAt(index - 1);
  const current = text.charCodeAt(index);
  return (
    previous >= 0xd800 &&
    previous <= 0xdbff &&
    current >= 0xdc00 &&
    current <= 0xdfff
  );
}

/**
 * Numeric-only observation of the native UTF-8 conversion for malformed UTF-16.
 * Returns the reported byte length, or -1 if the native call rejected it.
 * The synthetic source is never logged.
 */
async function probeNativeEncoding(
  scannerId: number,
  text: string,
): Promise<number> {
  try {
    const report = await NativeCribraReactNative.scan(
      scannerId,
      'probe',
      text,
    );
    return report.sourceBytes;
  } catch {
    return -1;
  }
}

/**
 * Measures the O(n) preflight over a 1 MiB ASCII source. Reported as a numeric
 * observation only; it never gates the smoke result.
 */
function measurePreflightMs(): number | null {
  const perf = (
    globalThis as { performance?: { now(): number } }
  ).performance;
  if (perf === undefined) {
    return null;
  }
  const source = 'a'.repeat(1024 * 1024);
  assertWellFormedUtf16(source);
  const runs = 5;
  const started = perf.now();
  for (let index = 0; index < runs; index += 1) {
    assertWellFormedUtf16(source);
  }
  return Number(((perf.now() - started) / runs).toFixed(3));
}

/**
 * RN.1.3 configuration proof. Every success case is compared against the
 * direct-Core oracle for both rule metadata and, where present, scan output.
 * Every Core build failure is checked for its structured code and rule ID.
 * No matcher value or rule ID is emitted, and the registry returns to baseline.
 */
async function runConfigChecks(): Promise<string | null> {
  const baseline = getLiveScannerCount();

  // Explicit empty configuration: no catalogs, no rules.
  const empty = await createScanner({ builtins: 'none' });
  if (
    empty.rulesCount !== 0 ||
    empty.rules.length !== 0 ||
    !Object.isFrozen(empty.rules) ||
    getLiveScannerCount() !== baseline + 1
  ) {
    await empty.dispose();
    return 'config_empty';
  }
  await empty.dispose();
  if (empty.rulesCount !== 0 || !Object.isFrozen(empty.rules)) {
    return 'config_empty_after';
  }
  if (getLiveScannerCount() !== baseline) {
    return 'config_empty_dispose';
  }

  const successNames = [
    'default',
    'explicit_empty',
    'financial_only',
    'personal_only',
    'all_catalogs',
    'custom_literal',
    'custom_prefix',
    'custom_suffix',
    'custom_pattern',
    'custom_all_kinds',
    'custom_before_catalog',
  ];
  for (const name of successNames) {
    const testCase = configCaseByName.get(name);
    const expected = configOracleByName.get(name);
    if (testCase === undefined || expected === undefined || expected.error) {
      return `config_fixture:${name}`;
    }

    const scanner = await createScanner(scannerConfigFor(testCase));
    if (getLiveScannerCount() !== baseline + 1) {
      await scanner.dispose();
      return `config_count:${name}`;
    }

    const metadataDiff = diffJson(scanner.rules, oracleMetadata(expected));
    if (metadataDiff !== null) {
      await scanner.dispose();
      return `config_metadata:${name}:${metadataDiff}`;
    }

    if (testCase.scan !== undefined && testCase.scan.length > 0) {
      const batch = await scanner.scanBatch(
        testCase.scan.map(({ id, text }) => ({ id, source: text })),
      );
      const scanDiff = diffJson(
        { entries: batch.entries, summary: batch.summary },
        { entries: expected.entries, summary: expected.summary },
      );
      if (scanDiff !== null) {
        await scanner.dispose();
        return `config_scan:${name}:${scanDiff}`;
      }
    }

    await scanner.dispose();
    if (getLiveScannerCount() !== baseline) {
      return `config_dispose:${name}`;
    }
    if (
      scanner.rulesCount !== (expected.rules ?? []).length ||
      !Object.isFrozen(scanner.rules)
    ) {
      return `config_after:${name}`;
    }
  }

  for (const [name, expected] of configOracleByName) {
    if (expected.error === undefined) {
      continue;
    }
    const testCase = configCaseByName.get(name);
    if (testCase === undefined) {
      return `config_fixture:${name}`;
    }

    let code: string | undefined;
    let ruleId: string | undefined;
    let message = '';
    try {
      const scanner = await createScanner(scannerConfigFor(testCase));
      await scanner.dispose();
      return `config_expected_error:${name}`;
    } catch (error) {
      if (!(error instanceof CribraError)) {
        return `config_error_type:${name}`;
      }
      code = error.code;
      ruleId = error.ruleId;
      message = error.message;
    }

    const expectedCode = BUILD_ERROR_LABELS[expected.error.code - 1];
    if (code !== expectedCode) {
      return `config_error_code:${name}`;
    }
    if ((ruleId ?? '') !== (expected.error.ruleId ?? '')) {
      return `config_error_rule:${name}`;
    }
    const expectedRuleId = expected.error.ruleId;
    if (
      expectedRuleId !== null &&
      expectedRuleId !== '' &&
      message.includes(expectedRuleId)
    ) {
      return `config_error_leak:${name}`;
    }
    if (getLiveScannerCount() !== baseline) {
      return `config_error_count:${name}`;
    }
  }

  return null;
}

/**
 * Privacy-safe fragments observed during transformation checks. Only their
 * absence from the emitted marker is asserted; they are never logged.
 */
const transformPrivacyFragments: string[] = [];

function transformRequestFor(testCase: TransformCase): TransformRequest {
  const key =
    testCase.key === undefined ? undefined : Uint8Array.from(testCase.key);
  const request = testCase.request;
  switch (request.kind) {
    case 'redact':
      return request.replacement === undefined
        ? { kind: 'redact' }
        : { kind: 'redact', replacement: request.replacement };
    case 'template':
      return {
        kind: 'template',
        ...(request.namespace === undefined
          ? {}
          : { namespace: request.namespace }),
        ...(request.numbered === undefined
          ? {}
          : { numbered: request.numbered }),
      };
    case 'pseudonymize':
      if (key === undefined) {
        throw new Error('transform fixture key missing');
      }
      return {
        kind: 'pseudonymize',
        key,
        ...(request.prefix === undefined ? {} : { prefix: request.prefix }),
        ...(request.digestBytes === undefined
          ? {}
          : { digestBytes: request.digestBytes }),
      };
    case 'synthesize':
      if (key === undefined) {
        throw new Error('transform fixture key missing');
      }
      return {
        kind: 'synthesize',
        key,
        ...(request.marker === undefined ? {} : { marker: request.marker }),
      };
  }
}

/**
 * RN.1.5 scan-coupled transformation proof. Every success case is compared to
 * the direct-Core oracle for both the detached scan projection and the exact
 * transformed output. Overlapping findings must reject in non-redact modes.
 * Key transport, malformed UTF-16, native key length, in-flight disposal and
 * deterministic keyed output are checked separately.
 */
async function runTransformChecks(): Promise<string | null> {
  const baseline = getLiveScannerCount();
  const outputs = new Map<string, string>();

  for (const [name, expected] of transformOracleByName) {
    const testCase = transformCaseByName.get(name);
    if (testCase === undefined) {
      return `transform_fixture:${name}`;
    }

    const scanner = await createScanner(
      scannerConfigFor(testCase as unknown as ConfigCase),
    );
    if (getLiveScannerCount() !== baseline + 1) {
      await scanner.dispose();
      return `transform_count:${name}`;
    }

    try {
      if (expected.error !== undefined) {
        let code: string | undefined;
        try {
          await scanner.transform(
            testCase.source,
            transformRequestFor(testCase),
          );
        } catch (error) {
          if (error instanceof CribraError) {
            code = error.code;
          }
        }
        if (code !== expected.error) {
          return `transform_error:${name}`;
        }
      } else {
        const result = await scanner.transform(
          testCase.source,
          transformRequestFor(testCase),
        );
        const diff = diffJson(
          { scan: result.scan, output: result.output },
          { scan: expected.scan, output: expected.output },
        );
        if (diff !== null) {
          return `transform_parity:${name}:${diff}`;
        }

        // The transform's report must describe the same source as a detached
        // scan of that source.
        const detached = await scanner.scan(testCase.source);
        const scanDiff = diffJson(detached, result.scan);
        if (scanDiff !== null) {
          return `transform_scan_parity:${name}:${scanDiff}`;
        }

        outputs.set(name, result.output);
        if (result.output.length >= 4) {
          transformPrivacyFragments.push(result.output);
        }
      }
    } finally {
      await scanner.dispose();
    }

    if (getLiveScannerCount() !== baseline) {
      return `transform_dispose:${name}`;
    }
  }

  if (
    outputs.get('pseudonymize_default') !==
    outputs.get('pseudonymize_repeat')
  ) {
    return 'transform_pseudonymize_determinism';
  }
  if (outputs.get('synthesize_default') !== outputs.get('synthesize_repeat')) {
    return 'transform_synthesize_determinism';
  }

  // Native key-length validation happens synchronously on the JS thread and
  // rejects before any worker job. Only source-free diagnostics are observed.
  const rawId = await NativeCribraReactNative.createDefaultScanner();
  if (getLiveScannerCount() !== baseline + 1) {
    await NativeCribraReactNative.disposeScanner(rawId);
    return 'transform_key_count';
  }
  const clean = 'ordinary synthetic text';
  let rejected31 = false;
  let rejected33 = false;
  let rejectedSynth = false;
  try {
    await NativeCribraReactNative.transformPseudonymize(
      rawId,
      clean,
      new ArrayBuffer(31),
      'p',
      16,
    );
  } catch (error) {
    rejected31 = isNativePrefix(
      error,
      'cribra_native_error:invalid_key_length',
    );
  }
  try {
    await NativeCribraReactNative.transformPseudonymize(
      rawId,
      clean,
      new ArrayBuffer(33),
      'p',
      16,
    );
  } catch (error) {
    rejected33 = isNativePrefix(
      error,
      'cribra_native_error:invalid_key_length',
    );
  }
  try {
    await NativeCribraReactNative.transformSynthesize(
      rawId,
      clean,
      new ArrayBuffer(31),
      'm',
    );
  } catch (error) {
    rejectedSynth = isNativePrefix(
      error,
      'cribra_native_error:invalid_key_length',
    );
  }
  if (!rejected31 || !rejected33 || !rejectedSynth) {
    await NativeCribraReactNative.disposeScanner(rawId);
    return 'transform_native_key';
  }

  const zeroKey = new Uint8Array(32).buffer;
  const cleanPseudo = await NativeCribraReactNative.transformPseudonymize(
    rawId,
    clean,
    zeroKey,
    'p',
    16,
  );
  const cleanSynth = await NativeCribraReactNative.transformSynthesize(
    rawId,
    clean,
    zeroKey,
    'm',
  );
  await NativeCribraReactNative.disposeScanner(rawId);
  if (
    cleanPseudo.output !== clean ||
    cleanSynth.output !== clean ||
    getLiveScannerCount() !== baseline
  ) {
    return 'transform_clean_native';
  }

  // Facade preflight: malformed UTF-16 source/options and invalid key length
  // are rejected before native work, and the scanner remains usable.
  const edge = await createScanner();
  try {
    let badSource = false;
    let badReplacement = false;
    let badPrefix = false;
    let badMarker = false;
    let badKey = false;
    try {
      await edge.transform('a\uD83Db', { kind: 'redact' });
    } catch (error) {
      badSource = isCribraError(error, 'invalid_unicode_string');
    }
    try {
      await edge.transform('ok', {
        kind: 'redact',
        replacement: '\uDC00',
      });
    } catch (error) {
      badReplacement = isCribraError(error, 'invalid_unicode_string');
    }
    try {
      await edge.transform('ok', {
        kind: 'pseudonymize',
        key: new Uint8Array(32),
        prefix: 'a\uD83D',
      });
    } catch (error) {
      badPrefix = isCribraError(error, 'invalid_unicode_string');
    }
    try {
      await edge.transform('ok', {
        kind: 'synthesize',
        key: new Uint8Array(32),
        marker: '\uDC00',
      });
    } catch (error) {
      badMarker = isCribraError(error, 'invalid_unicode_string');
    }
    try {
      await edge.transform('ok', {
        kind: 'pseudonymize',
        key: new Uint8Array(31),
      });
    } catch (error) {
      badKey = isCribraError(error, 'invalid_key_length');
    }
    if (!badSource || !badReplacement || !badPrefix || !badMarker || !badKey) {
      return 'transform_preflight';
    }
  } finally {
    await edge.dispose();
  }
  if (getLiveScannerCount() !== baseline) {
    return 'transform_edge_dispose';
  }

  // Disposal while a transformation is in flight: the queued serial job
  // completes first, then native disposal drains the registry.
  const inFlight = await createScanner();
  const pending = inFlight.transform(
    "GITHUB_TOKEN=ghp_AbCdEf0123456789_AbCdEf0123456789\n",
    { kind: 'redact' },
  );
  const pendingDispose = inFlight.dispose();
  const [inFlightResult] = await Promise.all([pending, pendingDispose]);
  if (
    getLiveScannerCount() !== baseline ||
    !inFlightResult.output.includes('[REDACTED]') ||
    inFlightResult.scan.findings.length !== 1
  ) {
    return 'transform_inflight_disposal';
  }

  return null;
}

async function runHarness(): Promise<Outcome> {
  const abi = getAbiVersion();
  const abiText = `${abi.major}.${abi.minor}.${abi.patch}`;
  if (
    abi.major !== oracle.abiVersion.major ||
    abi.minor !== oracle.abiVersion.minor ||
    abi.patch !== oracle.abiVersion.patch
  ) {
    return { state: 'fail', reason: 'abi' };
  }

  if (getLiveScannerCount() !== 0) {
    return { state: 'fail', reason: 'initial_count' };
  }

  const scanner = await createScanner();
  if (getLiveScannerCount() !== 1) {
    await scanner.dispose();
    return { state: 'fail', reason: 'create_count' };
  }

  // 1. Full-fixture parity: empty/duplicate/empty ids, Unicode, multiple
  //    findings, a candidate, ordering, and every projection field.
  const batch = await scanner.scanBatch(sources);
  const diff = diffJson(
    { entries: batch.entries, summary: batch.summary },
    { entries: oracle.entries, summary: oracle.summary },
  );
  if (diff !== null) {
    await scanner.dispose();
    return { state: 'fail', reason: `parity:${diff}` };
  }

  // 2. Identifiers and order survive verbatim, including duplicates.
  if (batch.entries.length !== sources.length) {
    await scanner.dispose();
    return { state: 'fail', reason: 'batch_length' };
  }
  for (let index = 0; index < sources.length; index += 1) {
    if (batch.entries[index].id !== sources[index].id) {
      await scanner.dispose();
      return { state: 'fail', reason: `batch_id:${index}` };
    }
  }

  // 3. Byte lengths are UTF-8, independent of JavaScript string length.
  let expectedBytes = 0;
  for (let index = 0; index < sources.length; index += 1) {
    const expected = utf8ByteLength(sources[index].source);
    expectedBytes += expected;
    if (
      batch.entries[index].sourceBytes !== expected ||
      batch.entries[index].result.sourceBytes !== expected
    ) {
      await scanner.dispose();
      return { state: 'fail', reason: `utf8_length:${index}` };
    }
  }
  if (batch.summary.scannedBytes !== expectedBytes) {
    await scanner.dispose();
    return { state: 'fail', reason: 'utf8_total' };
  }

  // 3b. Exact UTF-8 byte <-> UTF-16 correspondence for every reported span.
  //     Single-span and batch conversions must agree, the reverse conversion
  //     must round-trip, the sliced text must have the exact byte length, and
  //     no range may split a surrogate pair.
  for (let index = 0; index < batch.entries.length; index += 1) {
    const text = sources[index].source;
    const result = batch.entries[index].result;
    const spans = [
      ...result.findings.map((finding) => finding.span),
      ...result.candidates.map((candidate) => candidate.span),
    ];
    if (spans.length === 0) {
      continue;
    }
    const batchRanges = byteSpansToUtf16Ranges(text, spans);
    for (let spanIndex = 0; spanIndex < spans.length; spanIndex += 1) {
      const span = spans[spanIndex];
      const range = byteSpanToUtf16Range(text, span);
      if (
        range.start !== batchRanges[spanIndex].start ||
        range.end !== batchRanges[spanIndex].end
      ) {
        await scanner.dispose();
        return { state: 'fail', reason: `offset_batch:${index}:${spanIndex}` };
      }
      const roundTrip = utf16RangeToByteSpan(text, range);
      if (roundTrip.start !== span.start || roundTrip.end !== span.end) {
        await scanner.dispose();
        return {
          state: 'fail',
          reason: `offset_reverse:${index}:${spanIndex}`,
        };
      }
      const slice = text.slice(range.start, range.end);
      if (utf8ByteLength(slice) !== span.end - span.start) {
        await scanner.dispose();
        return {
          state: 'fail',
          reason: `offset_length:${index}:${spanIndex}`,
        };
      }
      if (
        splitsSurrogatePair(text, range.start) ||
        splitsSurrogatePair(text, range.end)
      ) {
        await scanner.dispose();
        return {
          state: 'fail',
          reason: `offset_split:${index}:${spanIndex}`,
        };
      }
    }
  }

  // 4. An empty batch is one native operation with an exact zero summary.
  const empty = await scanner.scanBatch([]);
  if (!isZeroSummary(empty)) {
    await scanner.dispose();
    return { state: 'fail', reason: 'empty_batch' };
  }

  // 5. Repeating the same ordered batch is deterministic.
  const repeated = await scanner.scanBatch(sources);
  if (
    diffJson(
      { entries: repeated.entries, summary: repeated.summary },
      { entries: batch.entries, summary: batch.summary },
    ) !== null
  ) {
    await scanner.dispose();
    return { state: 'fail', reason: 'repeat_determinism' };
  }

  // 6. Invalid JavaScript input is rejected before any native work, and the
  //    scanner remains usable afterwards.
  let invalidJs = false;
  try {
    await scanner.scanBatch([
      { id: 1, source: 'unused' },
    ] as unknown as ScanSource[]);
  } catch (error) {
    invalidJs = isCribraError(error, 'invalid_argument');
  }
  if (!invalidJs) {
    await scanner.dispose();
    return { state: 'fail', reason: 'invalid_js_batch' };
  }
  if (!isZeroSummary(await scanner.scanBatch([]))) {
    await scanner.dispose();
    return { state: 'fail', reason: 'post_invalid_batch' };
  }

  // 6b. Malformed UTF-16 is rejected by the public scan paths before any native
  //     work. The scanner stays usable, and a valid supplementary source still
  //     reports its exact UTF-8 byte length.
  let malformedScan = false;
  try {
    await scanner.scan('a\uD83Db');
  } catch (error) {
    malformedScan = isCribraError(error, 'invalid_unicode_string');
  }
  if (!malformedScan) {
    await scanner.dispose();
    return { state: 'fail', reason: 'preflight_scan' };
  }
  let malformedBatch = false;
  try {
    await scanner.scanBatch([
      { id: 'ok', source: 'plain' },
      { id: 'bad', source: '\uDC00' },
    ]);
  } catch (error) {
    malformedBatch = isCribraError(error, 'invalid_unicode_string');
  }
  if (!malformedBatch) {
    await scanner.dispose();
    return { state: 'fail', reason: 'preflight_batch' };
  }
  const supplementary = await scanner.scan('a🙂b');
  if (supplementary.sourceBytes !== 6) {
    await scanner.dispose();
    return { state: 'fail', reason: 'preflight_valid_supplementary' };
  }
  if (!isZeroSummary(await scanner.scanBatch([]))) {
    await scanner.dispose();
    return { state: 'fail', reason: 'preflight_after' };
  }

  // 7. Concurrent disposal shares one native disposal and resolves after it.
  await Promise.all([scanner.dispose(), scanner.dispose()]);
  if (getLiveScannerCount() !== 0) {
    return { state: 'fail', reason: 'dispose_count' };
  }

  // 8. dispose() is contractually idempotent once complete.
  await scanner.dispose();

  // 9. A disposed facade rejects batch work without touching the native side.
  let disposed = false;
  try {
    await scanner.scanBatch(sources);
  } catch (error) {
    disposed = isCribraError(error, 'disposed');
  }
  if (!disposed) {
    return { state: 'fail', reason: 'post_dispose_batch' };
  }

  // 10. Deterministic native negatives: an unknown scanner id and malformed
  //     batch elements fail closed, and the scanner stays alive afterwards.
  let invalidId = false;
  try {
    await NativeCribraReactNative.scanBatch(999999, []);
  } catch (error) {
    invalidId = isNativePrefix(error, 'cribra_native_error:scanner_not_found');
  }
  if (!invalidId) {
    return { state: 'fail', reason: 'native_invalid_id' };
  }

  const rawId = await NativeCribraReactNative.createDefaultScanner();
  if (getLiveScannerCount() !== 1) {
    await NativeCribraReactNative.disposeScanner(rawId);
    return { state: 'fail', reason: 'raw_create_count' };
  }
  let malformed = false;
  try {
    await NativeCribraReactNative.scanBatch(
      rawId,
      [{ id: 1, source: 2 }] as unknown as { id: string; source: string }[],
    );
  } catch (error) {
    malformed = isNativePrefix(error, 'cribra_native_error:invalid_argument');
  }
  if (!malformed) {
    await NativeCribraReactNative.disposeScanner(rawId);
    return { state: 'fail', reason: 'native_malformed_args' };
  }
  const rawEmpty = await NativeCribraReactNative.scanBatch(rawId, []);
  if (
    rawEmpty.entries.length !== 0 ||
    rawEmpty.summary.scannedSources !== 0 ||
    !rawEmpty.summary.isClean
  ) {
    await NativeCribraReactNative.disposeScanner(rawId);
    return { state: 'fail', reason: 'native_after_malformed' };
  }
  // 10b. Numeric-only probe of the native UTF-8 conversion for malformed UTF-16.
  //      Hermes replaces with U+FFFD (3 bytes); JSC truncates. Only lengths are
  //      observed and emitted, never the synthetic text.
  const probeLone = await probeNativeEncoding(rawId, '\uD83D');
  const probeMiddle = await probeNativeEncoding(rawId, 'a\uD83Db');
  const probe = `${probeLone}/${probeMiddle}`;
  await NativeCribraReactNative.disposeScanner(rawId);
  if (getLiveScannerCount() !== 0) {
    return { state: 'fail', reason: 'raw_dispose_count' };
  }

  // 11. Disposal while a batch is in flight: the queued serial job completes
  //     first, then the native disposal runs and the registry drains.
  const inFlight = await createScanner();
  const pendingBatch = inFlight.scanBatch(sources);
  const pendingDispose = inFlight.dispose();
  const [inFlightBatch] = await Promise.all([pendingBatch, pendingDispose]);
  if (
    inFlightBatch.entries.length !== sources.length ||
    inFlightBatch.summary.totalFindings !==
      batch.summary.totalFindings ||
    getLiveScannerCount() !== 0
  ) {
    return { state: 'fail', reason: 'inflight_disposal' };
  }

  // 12. Repeat creation/disposal to prove the registry can be reused.
  const second = await createScanner();
  if (getLiveScannerCount() !== 1) {
    await second.dispose();
    return { state: 'fail', reason: 'second_create_count' };
  }
  await second.dispose();
  if (getLiveScannerCount() !== 0) {
    return { state: 'fail', reason: 'second_dispose_count' };
  }

  // 13. Configured construction, detached metadata, and structured build
  //     failures against the direct-Core oracle.
  const configFailure = await runConfigChecks();
  if (configFailure !== null) {
    return { state: 'fail', reason: configFailure };
  }

  const transformFailure = await runTransformChecks();
  if (transformFailure !== null) {
    return { state: 'fail', reason: transformFailure };
  }

  const stressFailure = await runStressChecks();
  if (stressFailure !== null) {
    return { state: 'fail', reason: stressFailure };
  }

  const recoveryFailure = await runRecoveryChecks();
  if (recoveryFailure !== null) {
    return { state: 'fail', reason: recoveryFailure };
  }

  const preflightMs = measurePreflightMs();
  if (preflightMs !== null) {
    // Numeric-only observation; never gates the smoke result.
    console.log(
      `CRIBRA_RN_PREFLIGHT:ms=${preflightMs}:bytes=${1024 * 1024}`,
    );
  }

  const result: Outcome = {
    state: 'pass',
    abi: abiText,
    entries: batch.entries.length,
    findings: batch.summary.totalFindings,
    candidates: batch.summary.totalCandidates,
    probe,
  };

  // 14. Privacy: the emitted marker contains no fixture token, candidate value,
  //     matcher payload, rule ID, or source identifier.
  const markerText = marker(result) ?? '';
  for (const fragment of secretFragments) {
    if (markerText.includes(fragment)) {
      return { state: 'fail', reason: 'privacy_secret' };
    }
  }
  if (markerText.includes('custom.')) {
    return { state: 'fail', reason: 'privacy_rule_id' };
  }
  for (const source of sources) {
    if (source.id.length > 0 && markerText.includes(source.id)) {
      return { state: 'fail', reason: 'privacy_id' };
    }
  }
  for (const fragment of transformPrivacyFragments) {
    if (markerText.includes(fragment)) {
      return { state: 'fail', reason: 'privacy_transform' };
    }
  }

  return result;
}

// ---------------------------------------------------------------------------
// RN.1.6 lifecycle stress: one thousand concurrent short/Unicode/empty scans,
// one ordered batch, all four transformation modes, and repeated keyed
// transforms. Only counts and durations are emitted.
// ---------------------------------------------------------------------------
const kStressScans = 1000;
const kStressKeyedTransforms = 200;

const stressTexts = [
  '',
  'short message',
  'contact a@b.co',
  'a🙂b',
  'line\nbreak',
  '0123456789',
];

async function runStressChecks(): Promise<string | null> {
  const stress = await createScanner();
  const key = new Uint8Array(32);
  for (let index = 0; index < key.length; index += 1) {
    key[index] = (index * 7 + 1) & 0xff;
  }
  const keySnapshot = [...key];

  try {
    const started = Date.now();
    const results = await Promise.all(
      Array.from({ length: kStressScans }, (_value, index) =>
        stress.scan(stressTexts[index % stressTexts.length], {
          id: `stress-${index % 8}`,
        }),
      ),
    );
    const elapsed = Date.now() - started;

    for (let index = 0; index < results.length; index += 1) {
      const text = stressTexts[index % stressTexts.length];
      if (results[index].sourceBytes !== utf8ByteLength(text)) {
        await stress.dispose();
        return `stress_scan:${index % stressTexts.length}`;
      }
    }
    console.log(`CRIBRA_RN_STRESS:scans=${kStressScans}:ms=${elapsed}`);

    const stressBatch = await stress.scanBatch(
      stressTexts.map((source, index) => ({
        id: `stress-batch-${index}`,
        source,
      })),
    );
    if (
      stressBatch.entries.length !== stressTexts.length ||
      stressBatch.summary.scannedSources !== stressTexts.length
    ) {
      await stress.dispose();
      return 'stress_batch';
    }

    const modes: TransformRequest[] = [
      { kind: 'redact' },
      { kind: 'template', namespace: 'STRESS' },
      { kind: 'pseudonymize', key, digestBytes: 8 },
      { kind: 'synthesize', key },
    ];
    for (const request of modes) {
      const transformed = await stress.transform('stress message', request);
      if (typeof transformed.output !== 'string') {
        await stress.dispose();
        return `stress_transform:${request.kind}`;
      }
    }

    const keyedStarted = Date.now();
    await Promise.all(
      Array.from({ length: kStressKeyedTransforms }, (_value, index) =>
        stress.transform(
          `keyed-${index % 4}`,
          index % 2 === 0
            ? { kind: 'pseudonymize', key, digestBytes: 8 }
            : { kind: 'synthesize', key },
        ),
      ),
    );
    const keyedElapsed = Date.now() - keyedStarted;
    console.log(
      `CRIBRA_RN_STRESS:keyed=${kStressKeyedTransforms}:ms=${keyedElapsed}`,
    );

    if (key.some((value, index) => value !== keySnapshot[index])) {
      await stress.dispose();
      return 'stress_key_mutation';
    }
    if (getLiveScannerCount() !== 1) {
      await stress.dispose();
      return 'stress_count';
    }
    await stress.dispose();
    if (getLiveScannerCount() !== 0) {
      return 'stress_dispose';
    }
    return null;
  } catch (error) {
    return error instanceof CribraError
      ? `stress:${error.code}`
      : 'stress:exception';
  }
}

// ---------------------------------------------------------------------------
// RN.1.6 recovery: an isolated rejected native job must not disturb later FIFO
// work, and the registry must still drain afterwards.
// ---------------------------------------------------------------------------
const kRecoveryRounds = 8;
const missingScannerId = 987654321;

async function runRecoveryChecks(): Promise<string | null> {
  const scanner = await createScanner();
  const probeText = 'recovery probe';
  try {
    const baseline = await scanner.scan(probeText);

    for (let round = 0; round < kRecoveryRounds; round += 1) {
      let sawFailure = false;
      try {
        await NativeCribraReactNative.disposeScanner(missingScannerId);
      } catch (error) {
        sawFailure = isNativePrefix(
          error,
          'cribra_native_error:scanner_not_found',
        );
      }
      if (!sawFailure) {
        await scanner.dispose();
        return `recovery_failure:${round}`;
      }

      const after = await scanner.scan(probeText);
      if (after.sourceBytes !== baseline.sourceBytes) {
        await scanner.dispose();
        return `recovery_after:${round}`;
      }
    }

    const finalBatch = await scanner.scanBatch([
      { id: 'recovery', source: 'after isolated failures' },
      { id: 'recovery-empty', source: '' },
    ]);
    if (finalBatch.entries.length !== 2) {
      await scanner.dispose();
      return 'recovery_batch';
    }

    await scanner.dispose();
    if (getLiveScannerCount() !== 0) {
      return 'recovery_count';
    }
    return null;
  } catch (error) {
    return error instanceof CribraError
      ? `recovery:${error.code}`
      : 'recovery:exception';
  }
}

function App() {
  const [outcome, setOutcome] = useState<Outcome>({ state: 'running' });
  const markerText = marker(outcome);

  useEffect(() => {
    let active = true;
    runHarness()
      .then((result) => {
        if (active) {
          setOutcome(result);
          console.log(marker(result));
        }
      })
      .catch((error: unknown) => {
        if (active) {
          const reason =
            error instanceof CribraError ? error.code : 'exception';
          const result: Outcome = { state: 'fail', reason };
          setOutcome(result);
          console.log(marker(result));
        }
      });
    return () => {
      active = false;
    };
  }, []);

  return (
    <View style={styles.container}>
      <StatusBar barStyle="dark-content" />
      <Text style={styles.title}>Cribra RN.1.6 lifecycle smoke</Text>
      <Text style={styles.status} testID="cribra-status">
        {outcome.state.toUpperCase()}
      </Text>
      {markerText ? (
        <Text style={styles.marker} testID="cribra-marker">
          {markerText}
        </Text>
      ) : null}
      {outcome.state === 'running' ? (
        <Text style={styles.body}>Running native scanner proof…</Text>
      ) : null}
      {outcome.state === 'pass' ? (
        <View>
          <Text style={styles.body}>ABI 0.3.0</Text>
          <Text style={styles.body}>Sources: {outcome.entries}</Text>
          <Text style={styles.body}>Findings: {outcome.findings}</Text>
          <Text style={styles.body}>Candidates: {outcome.candidates}</Text>
          <Text style={styles.body}>Parity: ok</Text>
          <Text style={styles.body}>Batch: ok</Text>
          <Text style={styles.body}>Config: ok</Text>
          <Text style={styles.body}>Transform: ok</Text>
          <Text style={styles.body}>Key transport: ok</Text>
          <Text style={styles.body}>Negatives: ok</Text>
          <Text style={styles.body}>Privacy: ok</Text>
          <Text style={styles.body}>Lifecycle: ok</Text>
          <Text style={styles.body}>Stress: ok</Text>
          <Text style={styles.body}>Recovery: ok</Text>
        </View>
      ) : null}
      {outcome.state === 'fail' ? (
        <Text style={styles.body}>Reason: {outcome.reason}</Text>
      ) : null}
      {__DEV__ ? (
        <Pressable
          accessibilityRole="button"
          onPress={() => DevSettings.reload()}
          style={styles.reloadButton}
        >
          <Text style={styles.reloadText}>Reload JS</Text>
        </Pressable>
      ) : null}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: '#f6f6f6',
    padding: 24,
  },
  title: {
    fontSize: 20,
    fontWeight: '600',
    marginBottom: 16,
  },
  status: {
    fontSize: 28,
    fontWeight: '700',
    marginBottom: 16,
  },
  marker: {
    fontSize: 10,
    fontFamily: 'Menlo',
    textAlign: 'center',
    marginBottom: 16,
  },
  body: {
    fontSize: 16,
    marginBottom: 4,
  },
  reloadButton: {
    marginTop: 16,
    paddingHorizontal: 16,
    paddingVertical: 8,
    borderRadius: 8,
    backgroundColor: '#1f6feb',
  },
  reloadText: {
    color: '#ffffff',
    fontSize: 14,
    fontWeight: '600',
  },
});

export default App;
