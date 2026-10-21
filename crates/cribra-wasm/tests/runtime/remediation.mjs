import assert from "node:assert/strict";
import { makeEngine, projectReport } from "./stream.mjs";

export function qualifyRemediation(mod, variant, cases, oracle) {
  assert.equal(cases.length, oracle.length);
  let comparisons = 0;
  for (let i = 0; i < cases.length; i++) {
    const spec = cases[i];
    const expected = oracle[i];
    assert.equal(spec.name, expected.name);
    const engine = makeEngine(mod, spec);
    const whole = engine.scan(spec.source);
    const stream = engine.stream();
    stream.push(new TextEncoder().encode(spec.source));
    const streamed = stream.finish();
    stream.free();
    const incremental = engine.incremental(spec.source);
    const snapshot = incremental.snapshot();
    const result = snapshot.result;
    incremental.free(); snapshot.free();
    for (const report of [whole, streamed, result]) {
      assert.deepEqual(projectReport(mod, engine, report), expected.report, spec.name);
      if (variant === "flow") assert.equal(report.redact(spec.source), whole.redact(spec.source));
      comparisons++;
    }
    result.free(); streamed.free(); whole.free(); engine.free();
  }
  const families = [["Literal", "SECRET"], ["Prefix", "pre_"], ["Suffix", "_END"], ["Pattern", "code42"]];
  for (const [family, value] of families) {
    const method = `add${family}WithRemediation`;
    const builder = new mod.ScanEngineBuilder(false);
    assert.throws(() => builder[method]("custom", value, mod.FindingSeverity.High, mod.RemediationKind.Unknown), /unknown remediation/);
    assert.equal(builder.customRuleCount(), 0);
    builder[method]("valid", value, mod.FindingSeverity.High, mod.RemediationKind.RotatePassword);
    assert.equal(builder.customRuleCount(), 1);
    builder.free();
    for (const [id, input] of [["", value], ...(family === "Pattern" ? [] : [["empty", ""]])]) {
      const invalid = new mod.ScanEngineBuilder(false);
      invalid[method](id, input, mod.FindingSeverity.High, mod.RemediationKind.RemoveSensitiveValue);
      assert.throws(() => invalid.build()); // consumes builder even on failure
    }
    const duplicate = new mod.ScanEngineBuilder(false);
    duplicate[method]("same", value, mod.FindingSeverity.High, mod.RemediationKind.ReviewSensitiveHash);
    duplicate.addLiteral("same", "SECOND", mod.FindingSeverity.Low);
    assert.throws(() => duplicate.build(), /duplicate rule identifier/);
    for (const [id, catalog] of [["github.classic-pat", "current"], ["financial.iban", "financial"], ["personal.us-ssn", "personal"]]) {
      const collision = new mod.ScanEngineBuilder(catalog === "current");
      if (catalog === "financial") collision.addFinancialBuiltins();
      if (catalog === "personal") collision.addPersonalBuiltins();
      collision[method](id, value, mod.FindingSeverity.High, mod.RemediationKind.RemoveSensitiveValue);
      assert.throws(() => collision.build(), /duplicate rule identifier/);
    }
  }
  const pattern = new mod.ScanEngineBuilder(false);
  for (const value of ["[", "a*", "\\b", "(?=a)"]) {
    let original;
    try { pattern.addPattern("invalid", value, mod.FindingSeverity.High); } catch (error) { original = error.message; }
    assert(original);
    assert.throws(() => pattern.addPatternWithRemediation("invalid", value, mod.FindingSeverity.High, mod.RemediationKind.Unknown),
      (error) => error instanceof Error && error.message === original);
    assert.equal(pattern.customRuleCount(), 0);
  }
  pattern.free();
  console.log(`${variant}: custom remediation passed ${comparisons} direct-Core scan/stream/incremental reports across ${cases.length} configurations; all families, all values, defaults and configuration errors`);
}
