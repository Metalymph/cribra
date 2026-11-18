import assert from "node:assert/strict";
import { before, test } from "node:test";

import {
  candidateSource,
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

test("batch preserves order, IDs, bytes, and the Core summary", () => {
  const scanner = api.createScanner(customScannerOptions());
  const sources = [
    { id: "", source: "clean" },
    { id: "", source: "token-1234" },
    { id: "unicode-🦀", source: customSource },
    { id: "unicode-🦀", source: candidateSource },
  ];
  try {
    const batch = scanner.scanBatch(sources);
    assert.equal(batch.entries.length, sources.length);
    assert.deepEqual(
      batch.entries.map((entry) => entry.id),
      ["", "", "unicode-🦀", "unicode-🦀"],
      "IDs are preserved verbatim, including empty and duplicate values",
    );
    assert.deepEqual(
      batch.entries.map((entry) => entry.sourceBytes),
      [
        new TextEncoder().encode("clean").length,
        new TextEncoder().encode("token-1234").length,
        new TextEncoder().encode(customSource).length,
        new TextEncoder().encode(candidateSource).length,
      ],
    );
    assert.equal(batch.entries[0].result.findingCount, 0);
    assert.equal(batch.entries[1].result.findingCount, 1);
    assert.equal(batch.entries[2].result.findingCount, 4);
    assert.equal(batch.entries[3].result.candidateCount, 1);
    assert.deepEqual(batch.entries[3].result.candidates[0].span, {
      start: 0,
      end: 19,
    });

    assert.deepEqual(batch.summary, {
      scannedSources: 4,
      scannedBytes:
        new TextEncoder().encode("clean").length +
        new TextEncoder().encode("token-1234").length +
        new TextEncoder().encode(customSource).length +
        new TextEncoder().encode(candidateSource).length,
      reportsWithFindings: 2,
      reportsWithoutFindings: 2,
      totalFindings: 5,
      totalCandidates: 1,
      reportsWithCandidates: 1,
      hasCandidates: true,
      critical: 2,
      high: 1,
      medium: 1,
      low: 1,
      info: 0,
      isClean: false,
      hasCritical: true,
    });
    plainData(batch);
    const json = JSON.stringify(batch);
    assert.equal(json.includes(customSource), false);
    assert.equal(json.includes("ABCD-EFGH-IJKL-MNOP"), false);
  } finally {
    scanner.dispose();
  }
});

test("empty batches are valid and explain options apply to every entry", () => {
  const scanner = api.createScanner(customScannerOptions());
  try {
    const empty = scanner.scanBatch([]);
    assert.deepEqual(empty.entries, []);
    assert.deepEqual(empty.summary, {
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
    });

    const explained = scanner.scanBatch(
      [{ id: "one", source: "token-1234" }],
      { includeExplanations: true },
    );
    assert.equal(explained.entries[0].result.findings[0].explanation.kind, "classified");
  } finally {
    scanner.dispose();
  }
});

test("batch results remain valid after scanner disposal", () => {
  const scanner = api.createScanner(customScannerOptions());
  const batch = scanner.scanBatch([{ id: "kept", source: "token-1234" }]);
  scanner.dispose();
  assert.equal(batch.entries[0].id, "kept");
  assert.equal(batch.entries[0].result.findingCount, 1);
  assert.equal(batch.summary.totalFindings, 1);
});
