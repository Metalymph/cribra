/**
 * Cribra RN.1.2 isolated consumer harness.
 *
 * This screen runs the synthetic-fixture parity check, the atomic batch
 * behaviours, the scanner lifecycle checks, and deterministic native negative
 * cases against the native TurboModule. It intentionally displays and logs only
 * aggregate counts and pass/fail markers; no source text or individual finding
 * metadata is logged.
 */

import { useEffect, useState } from 'react';
import { StatusBar, StyleSheet, Text, View } from 'react-native';
import {
  CribraError,
  createScanner,
  getAbiVersion,
  getLiveScannerCount,
  type BatchResult,
  type ScanSource,
} from '@metalymph/cribra-react-native';
// Direct TurboModule access is used only by the deterministic negative cases
// below (invalid scanner id and malformed arguments). The facade remains the
// public API under test everywhere else.
import NativeCribraReactNative from '../src/NativeCribraReactNative';

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

// Tokens and candidate-shaped values that must never appear in emitted logs.
const secretFragments = [
  'ghp_AbCdEf0123456789_AbCdEf0123456789',
  'sk_test_AbCdEf0123456789_AbCdEf0123456789',
  'ABCD-EFGH-IJKL-MNOP',
];

type Outcome =
  | { state: 'running' }
  | {
      state: 'pass';
      abi: string;
      entries: number;
      findings: number;
      candidates: number;
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
      'negative=ok',
      'privacy=ok',
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

  const result: Outcome = {
    state: 'pass',
    abi: abiText,
    entries: batch.entries.length,
    findings: batch.summary.totalFindings,
    candidates: batch.summary.totalCandidates,
  };

  // 13. Privacy: the emitted marker contains no fixture token, candidate value,
  //     or source identifier.
  const markerText = marker(result) ?? '';
  for (const fragment of secretFragments) {
    if (markerText.includes(fragment)) {
      return { state: 'fail', reason: 'privacy_secret' };
    }
  }
  for (const source of sources) {
    if (source.id.length > 0 && markerText.includes(source.id)) {
      return { state: 'fail', reason: 'privacy_id' };
    }
  }

  return result;
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
      <Text style={styles.title}>Cribra RN.1.2 native batch smoke</Text>
      <Text style={styles.status} testID="cribra-status">
        {outcome.state.toUpperCase()}
      </Text>
      {markerText ? (
        <Text style={styles.marker} testID="cribra-marker">
          {markerText}
        </Text>
      ) : null}
      {outcome.state === 'running' ? (
        <Text style={styles.body}>Running native batch proof…</Text>
      ) : null}
      {outcome.state === 'pass' ? (
        <View>
          <Text style={styles.body}>ABI 0.3.0</Text>
          <Text style={styles.body}>Sources: {outcome.entries}</Text>
          <Text style={styles.body}>Findings: {outcome.findings}</Text>
          <Text style={styles.body}>Candidates: {outcome.candidates}</Text>
          <Text style={styles.body}>Parity: ok</Text>
          <Text style={styles.body}>Batch: ok</Text>
          <Text style={styles.body}>Negatives: ok</Text>
          <Text style={styles.body}>Privacy: ok</Text>
          <Text style={styles.body}>Lifecycle: ok</Text>
        </View>
      ) : null}
      {outcome.state === 'fail' ? (
        <Text style={styles.body}>Reason: {outcome.reason}</Text>
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
});

export default App;
