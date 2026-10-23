// Actual generated Core/Flow sequential batch bindings against independent Core oracles.
import assert from "node:assert/strict";
import { makeEngine, projectReport } from "./stream.mjs";

export function projectSummary(mod, summary) {
  try {
    const fields = {
      scannedSources: summary.scannedSources,
      scannedBytes: summary.scannedBytes,
      reportsWithFindings: summary.reportsWithFindings,
      reportsWithoutFindings: summary.reportsWithoutFindings,
      totalFindings: summary.totalFindings,
      totalCandidates: summary.totalCandidates,
      reportsWithCandidates: summary.reportsWithCandidates,
      hasCandidates: summary.hasCandidates,
      critical: summary.critical,
      high: summary.high,
      medium: summary.medium,
      low: summary.low,
      info: summary.info,
      isClean: summary.isClean,
      hasCritical: summary.hasCritical,
    };
    for (const [name, field] of [
      ["Critical", "critical"], ["High", "high"], ["Medium", "medium"],
      ["Low", "low"], ["Info", "info"],
    ]) {
      assert.equal(summary.bySeverity(mod.FindingSeverity[name]), fields[field],
        `bySeverity(${name}) must match Core's aggregate field`);
    }
    return fields;
  } finally {
    summary.free();
  }
}

export function qualifyBatch(mod, variant, cases, oracle) {
  const encoder = new TextEncoder();
  const project = (engine, result) => projectReport(mod, engine, result);
  const transforms = (result, whole, source) => {
    if (variant !== "flow") return;
    const key = new Uint8Array(32).fill(11);
    const outcome = (operation) => {
      try { return { value: operation() }; }
      catch (error) {
        assert(error instanceof Error);
        return { error: error.message };
      }
    };
    for (const [method, args] of [["redact", []], ["redactWith", ["<hidden>"]], ["template", []],
      ["templateWith", ["SAFE", true]], ["pseudonymize", [key, "pseudo_", 16]], ["synthesize", [key, "synthetic"]]]) {
      assert.deepEqual(outcome(() => result[method](source, ...args)), outcome(() => whole[method](source, ...args)));
    }
  };

  let batches = 0;
  let entries = 0;
  let reports = 0;
  assert.equal(cases.length, oracle.length);
  for (let caseIndex = 0; caseIndex < cases.length; caseIndex++) {
    const spec = cases[caseIndex];
    const expected = oracle[caseIndex];
    assert.equal(expected.name, spec.name);

    const engine = makeEngine(mod, spec);
    const compatible = makeEngine(mod, spec);
    const batch = engine.scanBatch(spec.sources);
    assert(batch instanceof mod.ScanBatchResult);
    assert.equal(batch.length, spec.sources.length);
    assert.equal(batch.isEmpty(), spec.sources.length === 0);
    assert.deepEqual(projectSummary(mod, batch.summary()), expected.summary, `${spec.name}: Core summary`);
    batches++;

    const retained = [];
    for (let index = 0; index < spec.sources.length; index++) {
      const input = spec.sources[index];
      const entry = batch.entryAt(index);
      assert(entry instanceof mod.ScanBatchEntry);
      assert.equal(entry.id, input.id, `${spec.name}: ID preserved exactly at ${index}`);
      assert.equal(entry.sourceBytes, encoder.encode(input.source).length);
      assert.equal(entry.sourceBytes, expected.entries[index].report.sourceBytes);

      const result = entry.result;
      const second = entry.result;
      assert(result instanceof mod.ScanResult);
      assert(second instanceof mod.ScanResult);
      assert.notEqual(result, second, "each result access must be independently owned");
      assert.deepEqual(project(compatible, result), expected.entries[index].report, `${spec.name}: entry ${index}`);
      assert.deepEqual(project(compatible, second), expected.entries[index].report);

      // Single-source parity with the existing scan() API, for every batch member.
      const whole = compatible.scan(input.source);
      assert.deepEqual(project(compatible, result), project(compatible, whole), `${spec.name}: scan parity ${index}`);
      transforms(result, whole, input.source);
      whole.free();
      second.free();
      retained.push({ entry, result, input, expected: expected.entries[index].report });
      entries++;
      reports++;
    }

    assert.throws(() => batch.entryAt(spec.sources.length), `${spec.name}: entry index boundary`);
    assert.throws(() => batch.entryAt(-1), `${spec.name}: negative entry index`);

    // Batch results own their metadata, so the creating engine and batch can be
    // freed before entries, reports, summaries or repeated reads are used.
    const summary = batch.summary();
    engine.free();
    batch.free();
    assert.deepEqual(projectSummary(mod, summary), expected.summary, `${spec.name}: summary outlives batch`);
    for (const { entry, result, input, expected: report } of retained) {
      assert.equal(entry.id, input.id);
      assert.equal(entry.sourceBytes, encoder.encode(input.source).length);
      const late = entry.result;
      assert(late instanceof mod.ScanResult);
      assert.deepEqual(project(compatible, result), report);
      assert.deepEqual(project(compatible, late), report);
      assert.throws(() => late.findingAt(late.findingCount()), `${spec.name}: finding index boundary`);
      assert.throws(() => late.candidateAt(late.candidateCount()), `${spec.name}: candidate index boundary`);
      assert.throws(() => late.candidateExplanationAt(late.candidateCount()));
      assert.throws(() => compatible.explainFinding(late, late.findingCount()));
      late.free();
      result.free();
      entry.free();
    }
    compatible.free();
  }
  assert.equal(batches, cases.length);

  // Repeated scans produce independent batches, entries and reports.
  const rule = { builtins: false, rules: [{ kind: "literal", id: "literal", value: "SECRET", severity: "High" }] };
  const engine = makeEngine(mod, rule);
  const inputs = [{ id: "same", source: "é SECRET" }, { id: "", source: "" }];
  const first = engine.scanBatch(inputs);
  const second = engine.scanBatch(inputs);
  assert.notEqual(first, second);
  const firstEntry = first.entryAt(0);
  const secondEntry = second.entryAt(0);
  const firstResult = firstEntry.result;
  const secondResult = secondEntry.result;
  assert.notEqual(firstEntry, secondEntry);
  assert.notEqual(firstResult, secondResult);
  first.free();
  assert.equal(secondResult.findingCount(), 1);
  assert.equal(secondEntry.sourceBytes, encoder.encode(inputs[0].source).length);
  assert.equal(firstResult.findingCount(), 1);
  assert.equal(firstEntry.id, "same");
  firstResult.free(); secondResult.free(); firstEntry.free(); secondEntry.free(); second.free();

  // Invalid structural inputs fail without creating a partial batch. Empty and
  // Unicode IDs/sources remain valid and are covered by the fixtures above.
  for (const invalid of [null, undefined, "x", 1, true, {}, [], () => {}]) {
    assert.throws(() => engine.scanBatch([invalid]), `invalid batch element: ${String(invalid)}`);
  }
  for (const invalid of [{}, { id: "only-id" }, { source: "only-source" },
    { id: 1, source: "value" }, { id: "id", source: 1 }, { id: null, source: "value" },
    { id: "id", source: null }, { id: undefined, source: "value" }, { id: "id", source: undefined }]) {
    assert.throws(() => engine.scanBatch([invalid]), "invalid batch fields");
  }
  assert.throws(() => engine.scanBatch(null));
  assert.throws(() => engine.scanBatch("not an array"));
  const empty = engine.scanBatch([]);
  assert.equal(empty.length, 0);
  assert.equal(empty.isEmpty(), true);
  assert.throws(() => empty.entryAt(0));
  assert.equal(projectSummary(mod, empty.summary()).isClean, true);
  empty.free();

  // A batch can outlive its engine entirely, including its owned entries.
  const owner = makeEngine(mod, rule);
  const owned = owner.scanBatch([{ id: "独立", source: "🦀 SECRET" }]);
  const ownedEntry = owned.entryAt(0);
  owner.free();
  const ownedResult = ownedEntry.result;
  owned.free();
  assert.equal(ownedEntry.id, "独立");
  assert.equal(ownedResult.findingCount(), 1);
  assert.equal(ownedEntry.result.findingCount(), 1);
  ownedResult.free();
  ownedEntry.free();
  engine.free();

  console.log(`${variant}: batch passed ${batches} direct-Core batches, ${entries} ordered entries and ${reports} report comparisons; summary/scan parity, UTF-8 ordering, invalid inputs and ownership`);
}
