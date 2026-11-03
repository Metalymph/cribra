import assert from "node:assert/strict";
import { before, test } from "node:test";

import {
  customScannerOptions,
  customSource,
  loadPackage,
  plainData,
} from "../helpers.mjs";

let api;

before(async () => {
  api = await loadPackage();
  await api.init();
});

test("results are independent plain data that survive disposal", () => {
  const scanner = api.createScanner(customScannerOptions());
  const result = scanner.scan(customSource, { includeExplanations: true });
  const batch = scanner.scanBatch(
    [{ id: "kept", source: customSource }],
    { includeExplanations: true },
  );

  assert.deepEqual(Object.keys(scanner).sort(), [
    "dispose",
    "disposed",
    "rules",
    "rulesCount",
    "scan",
    "scanBatch",
  ]);
  assert.equal("free" in scanner, false);
  assert.equal(Object.isFrozen(scanner), true);
  assert.equal(Object.isFrozen(result), true);
  assert.equal(Object.isFrozen(result.findings), true);
  assert.equal(Object.isFrozen(result.findings[0]), true);
  assert.equal(Object.isFrozen(result.findings[0].span), true);
  assert.equal(Object.isFrozen(result.candidates), true);
  assert.equal(Object.isFrozen(batch), true);
  assert.equal(Object.isFrozen(batch.entries), true);
  assert.equal(Object.isFrozen(batch.summary), true);
  plainData(result);
  plainData(batch);

  scanner.dispose();
  scanner.dispose();
  assert.equal(scanner.disposed, true);
  assert.equal(result.findingCount, 4);
  assert.equal(result.findings[0].ruleId, "test.literal");
  assert.deepEqual(result.findings[0].span, { start: 0, end: 17 });
  assert.equal(result.findings[0].explanation.kind, "classified");
  assert.equal(batch.entries[0].id, "kept");
  assert.equal(batch.entries[0].result.findingCount, 4);
  assert.equal(batch.summary.totalFindings, 4);
  assert.equal(scanner.rules.length, 4);
  assert.equal(JSON.parse(JSON.stringify(result)).findingCount, 4);
});

test("every scan leaves no native handles in returned values", () => {
  const scanner = api.createScanner(customScannerOptions());
  try {
    for (let iteration = 0; iteration < 20; iteration += 1) {
      const result = scanner.scan(customSource, {
        includeExplanations: iteration % 2 === 0,
      });
      plainData(result);
    }
    const batch = scanner.scanBatch([
      { id: "a", source: customSource },
      { id: "b", source: "token-1234" },
    ]);
    plainData(batch);
  } finally {
    scanner.dispose();
  }
});
