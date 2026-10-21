// Native JS Error enrichment and owned Core metadata through actual generated bindings.
import assert from "node:assert/strict";
import { makeEngine } from "./stream.mjs";

export function qualifyConfiguration(mod, variant, cases, oracle) {
  const project = (views) => views.map((view) => ({
    ruleId: view.ruleId, kind: mod.RuleKind[view.kind], detectionMode: mod.DetectionModeKind[view.detectionMode],
    severity: mod.FindingSeverity[view.severity], remediation: view.remediation === mod.RemediationKind.None ? null : mod.RemediationKind[view.remediation],
  }));
  const checkError = (operation, expected) => {
    let actual;
    try { operation(); } catch (error) { actual = error; }
    assert(actual instanceof Error, "Legacy native JavaScript Error behavior must remain");
    assert.equal(actual.name, "Error");
    assert.equal(actual.code, mod.ScannerBuildErrorCode[expected.code]);
    assert.equal(actual.ruleId, expected.ruleId ?? undefined);
    assert.equal(actual.message, expected.message);
    assert.equal("free" in actual, false, "Enriched errors are garbage-collected");
    assert(!actual.message.includes("SCANNED_SECRET_DO_NOT_EXPOSE"));
  };
  assert.equal(cases.length, oracle.length);
  let successes = 0;
  let failures = 0;
  for (let i = 0; i < cases.length; i++) {
    const spec = cases[i]; const expected = oracle[i];
    assert.equal(spec.name, expected.name);
    if (expected.error) {
      checkError(() => makeEngine(mod, spec), expected.error);
      failures++;
      continue;
    }
    const engine = makeEngine(mod, spec);
    const first = engine.activeRules(); const second = engine.activeRules();
    assert(Array.isArray(first)); assert.notEqual(first, second);
    assert.equal(first.length, engine.rulesCount());
    assert.equal(first[first.length], undefined);
    assert.deepEqual(project(first), expected.rules);
    assert.deepEqual(project(second), expected.rules);
    for (let j = 0; j < first.length; j++) assert.notEqual(first[j], second[j]);
    const report = engine.scan("SCANNED_SECRET_DO_NOT_EXPOSE");
    report.free(); engine.free();
    assert.deepEqual(project(first), expected.rules, "Metadata must survive engine/report free");
    assert(!JSON.stringify(project(first)).includes("DO_NOT_EXPOSE"));
    for (const view of first) view.free();
    assert.deepEqual(project(second), expected.rules, "Separate calls own independent views");
    for (const view of second) view.free();
    successes++;
  }
  const prototype = Object.getOwnPropertyDescriptors(mod.RuleMetadataView.prototype);
  assert.deepEqual(Object.keys(prototype).filter((key) => prototype[key].get).sort(),
    ["detectionMode", "kind", "remediation", "ruleId", "severity"]);
  for (const key of ["matcher", "value", "pattern", "validator", "source"]) assert.equal(key in prototype, false);
  // Concrete configuration patterns may appear in Core diagnostics; scanned
  // source is never supplied to construction. This distinction is intentional.
  const builder = new mod.ScanEngineBuilder(false);
  let error;
  try { builder.addPattern("config", "CONFIG_PATTERN_SENTINEL[", mod.FindingSeverity.High); } catch (value) { error = value; }
  assert(error instanceof Error); assert.equal(error.code, mod.ScannerBuildErrorCode.InvalidPattern);
  assert(error.message.includes("CONFIG_PATTERN_SENTINEL"));
  assert.equal(builder.customRuleCount(), 0);
  builder.addLiteral("valid", "SECRET", mod.FindingSeverity.High);
  const engine = builder.build(); engine.free();
  for (const [method, value] of [["addLiteralWithRemediation", "SECRET"], ["addPrefixWithRemediation", "pre_"],
    ["addSuffixWithRemediation", "_END"], ["addPatternWithRemediation", "code42"]]) {
    const invalid = new mod.ScanEngineBuilder(false);
    checkError(() => invalid[method]("custom/clé-🔑", value, mod.FindingSeverity.High, mod.RemediationKind.Unknown), {
      code: "InvalidRemediation", ruleId: "custom/clé-🔑", message: "unknown remediation cannot configure a Core rule",
    });
    assert.equal(invalid.customRuleCount(), 0); invalid.free();
  }
  console.log(`${variant}: rule metadata passed ${successes} direct-Core configurations; ${failures} structured construction errors, owned views and legacy Error compatibility`);
}
