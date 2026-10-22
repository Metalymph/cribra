// Exercise raw JS numbers through actual wasm-bindgen glue, before i32 coercion.
import assert from "node:assert/strict";

const invalidNumbers = [
  -1, -0.5, -4294967296,
  0.5, 1.5, 4294967295.5,
  NaN, Infinity, -Infinity,
  4294967296, 4294967297, 2 ** 40,
  Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER + 1, 2 ** 64,
  -(Number.MAX_SAFE_INTEGER + 1),
];
const invalidTypes = [
  undefined, null, "0", true, false, 0n, {}, [], new Number(0),
  { valueOf() { assert.fail("index validation must not coerce objects"); } },
];
const invalidMessage = "index must be a nonnegative safe integer representable on this target";

export function qualifyIndices(mod, variant) {
  const builder = new mod.ScanEngineBuilder(false);
  builder.addLiteral("index.first", "INDEX_SECRET_A", mod.FindingSeverity.High);
  builder.addLiteral("index.second", "INDEX_SECRET_B", mod.FindingSeverity.High);
  const engine = builder.build();
  const source = "é INDEX_SOURCE_SENTINEL INDEX_SECRET_A\nABCD-EFGH-IJKL-MNOP INDEX_SECRET_B\nQRST-UVWX-YZ12-3456";
  const result = engine.scan(source);
  assert.equal(result.findingCount(), 2);
  assert.equal(result.candidateCount(), 2);
  const sources = [{ id: "first", source }, { id: "second", source: "ordinary" }];
  const batch = engine.scanBatch(sources);
  const explanation = (view) => [view.kind, view.detectionMode, view.candidateEvidence];
  const accessors = [
    {
      name: "ScanResult.findingAt", get: (index) => result.findingAt(index),
      project: (view) => [view.ruleId, view.start, view.end],
      expected: (index) => {
        const value = index === 0 ? "INDEX_SECRET_A" : "INDEX_SECRET_B";
        const start = new TextEncoder().encode(source.slice(0, source.indexOf(value))).length;
        return [index === 0 ? "index.first" : "index.second", start, start + value.length];
      },
      boundsMessage: "finding index out of range",
    },
    {
      name: "ScanResult.candidateAt", get: (index) => result.candidateAt(index),
      project: (view) => [view.kind, view.evidence, view.start, view.end],
      expected: (index) => {
        const value = index === 0 ? "ABCD-EFGH-IJKL-MNOP" : "QRST-UVWX-YZ12-3456";
        const start = new TextEncoder().encode(source.slice(0, source.indexOf(value))).length;
        return [mod.CandidateKind.RecoveryLikeCode, mod.CandidateEvidenceKind.Structural, start, start + value.length];
      },
      boundsMessage: "candidate index out of range",
    },
    {
      name: "ScanResult.candidateExplanationAt", get: (index) => result.candidateExplanationAt(index),
      project: explanation,
      expected: () => [mod.ExplanationKind.Ambiguous, undefined, mod.CandidateEvidenceKind.Structural],
      boundsMessage: "candidate index out of range",
    },
    {
      name: "ScanEngine.explainFinding", get: (index) => engine.explainFinding(result, index),
      project: explanation,
      expected: () => [mod.ExplanationKind.Classified, mod.DetectionModeKind.MatcherOnly, undefined],
      boundsMessage: "finding index out of range",
    },
    {
      name: "ScanBatchResult.entryAt", get: (index) => batch.entryAt(index),
      project: (view) => view.id, expected: (index) => sources[index].id,
      boundsMessage: "batch entry index out of range",
    },
  ];
  let bundle;
  if (variant === "flow") {
    const shareBuilder = new mod.ShareBundleBuilder();
    shareBuilder.modeRedact();
    bundle = shareBuilder.scanAndBuild(engine, sources);
    shareBuilder.free();
    accessors.push({
      name: "ShareBundle.entryAt", get: (index) => bundle.entryAt(index),
      project: (view) => view.id, expected: (index) => sources[index].id,
      boundsMessage: "share bundle entry index out of range",
    });
  }
  assert.equal(accessors.length, variant === "flow" ? 6 : 5);

  const checkError = (operation, message, context) => {
    assert.throws(operation, (error) => {
      assert(error instanceof Error, `${context}: native JS Error`);
      assert.equal(error.message, message, context);
      assert.equal("free" in error, false, `${context}: errors are garbage-collected`);
      for (const sensitive of [source, "INDEX_SOURCE_SENTINEL", "INDEX_SECRET_A", "INDEX_SECRET_B",
        "ABCD-EFGH-IJKL-MNOP", "QRST-UVWX-YZ12-3456"]) {
        assert(!String(error).includes(sensitive), `${context}: source-free diagnostic`);
      }
      return true;
    }, context);
  };
  const retained = [];
  for (const accessor of accessors) {
    // Independent views must survive invalid calls, freeing siblings and parents.
    const first = accessor.get(0);
    const second = accessor.get(1);
    retained.push({ accessor, first, second });
    assert.notEqual(first, second);
    for (const index of [0, -0, 1]) {
      const view = accessor.get(index);
      assert.notEqual(view, first);
      assert.deepEqual(accessor.project(view), accessor.expected(index), accessor.name);
      view.free();
    }
    for (const index of [...invalidNumbers, ...invalidTypes]) {
      checkError(() => accessor.get(index), invalidMessage, `${accessor.name}(${String(index)})`);
    }
    // Both are valid target-width numbers; only collection bounds reject them.
    for (const index of [2, 4294967295]) {
      checkError(() => accessor.get(index), accessor.boundsMessage, `${accessor.name} bounds`);
    }
    const after = accessor.get(1);
    assert.deepEqual(accessor.project(after), accessor.expected(1), `${accessor.name}: remains usable`);
    after.free();
  }
  bundle?.free();
  batch.free();
  result.free();
  engine.free();
  for (const { accessor, first, second } of retained) {
    assert.deepEqual(accessor.project(first), accessor.expected(0), `${accessor.name}: owned first`);
    first.free();
    assert.deepEqual(accessor.project(second), accessor.expected(1), `${accessor.name}: owned second`);
    second.free();
  }
  console.log(`${variant}: numeric indices passed ${accessors.length} accessors × (${invalidNumbers.length} invalid numbers + ${invalidTypes.length} non-number values); valid indices, bounds, privacy and independent ownership`);
}
