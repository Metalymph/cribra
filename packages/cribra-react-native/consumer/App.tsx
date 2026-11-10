/**
 * Cribra RN.1.1 isolated consumer harness.
 *
 * This screen runs the synthetic-fixture parity check and the scanner lifecycle
 * check against the native TurboModule. It intentionally displays and logs only
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
  type ScanSource,
} from '@metalymph/cribra-react-native';

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
    ].join(':');
  }
  if (outcome.state === 'fail') {
    return `CRIBRA_RN_SMOKE:FAIL:${outcome.reason}`;
  }
  return undefined;
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

  const batch = await scanner.scanBatch(sources);
  const diff = diffJson(
    { entries: batch.entries, summary: batch.summary },
    { entries: oracle.entries, summary: oracle.summary },
  );
  if (diff !== null) {
    await scanner.dispose();
    return { state: 'fail', reason: `parity:${diff}` };
  }

  // Concurrent disposal must share one native disposal and resolve after it.
  await Promise.all([scanner.dispose(), scanner.dispose()]);
  if (getLiveScannerCount() !== 0) {
    return { state: 'fail', reason: 'dispose_count' };
  }

  // dispose() is contractually idempotent once complete.
  await scanner.dispose();

  let disposed = false;
  try {
    await scanner.scan('not-scanned');
  } catch (error) {
    disposed = error instanceof CribraError && error.code === 'disposed';
  }
  if (!disposed) {
    return { state: 'fail', reason: 'post_dispose_scan' };
  }

  // Repeat creation/disposal to prove the registry can be reused.
  const second = await createScanner();
  if (getLiveScannerCount() !== 1) {
    await second.dispose();
    return { state: 'fail', reason: 'second_create_count' };
  }
  await second.dispose();
  if (getLiveScannerCount() !== 0) {
    return { state: 'fail', reason: 'second_dispose_count' };
  }

  return {
    state: 'pass',
    abi: abiText,
    entries: batch.entries.length,
    findings: batch.summary.totalFindings,
    candidates: batch.summary.totalCandidates,
  };
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
      <Text style={styles.title}>Cribra RN.1.1 native smoke</Text>
      <Text style={styles.status} testID="cribra-status">
        {outcome.state.toUpperCase()}
      </Text>
      {markerText ? (
        <Text style={styles.marker} testID="cribra-marker">
          {markerText}
        </Text>
      ) : null}
      {outcome.state === 'running' ? (
        <Text style={styles.body}>Running native scan proof…</Text>
      ) : null}
      {outcome.state === 'pass' ? (
        <View>
          <Text style={styles.body}>ABI 0.3.0</Text>
          <Text style={styles.body}>Sources: {outcome.entries}</Text>
          <Text style={styles.body}>Findings: {outcome.findings}</Text>
          <Text style={styles.body}>Candidates: {outcome.candidates}</Text>
          <Text style={styles.body}>Parity: ok</Text>
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
