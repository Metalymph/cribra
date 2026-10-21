// Streaming qualification shared by actual generated Core and Flow bindings.
import assert from "node:assert/strict";

export function qualifyStreams(mod, variant, cases, oracle) {
  const bytes = (text) => new TextEncoder().encode(text);
  const name = (table, value) => table[value];
  const make = (spec) => {
    if (spec.builtins) return new mod.ScanEngine();
    const builder = new mod.ScanEngineBuilder(false);
    for (const rule of spec.rules) {
      const method = { literal: "addLiteral", prefix: "addPrefix", suffix: "addSuffix", pattern: "addPattern" }[rule.kind];
      builder[method](rule.id, rule.value, mod.FindingSeverity[rule.severity]);
    }
    return builder.build(); // build consumes the builder
  };
  const literal = (value = "SECRET") => make({ rules: [{ id: "literal", kind: "literal", value, severity: "High" }] });
  const error = (operation, expected) => {
    let caught;
    try { operation(); } catch (value) { caught = value; }
    assert(caught instanceof mod.StreamError, `Expected typed StreamError (${expected})`);
    try {
      assert.equal(caught.code, mod.StreamErrorCode[expected]);
      assert.equal(typeof caught.message, "string");
      assert(caught.message.length > 0);
      for (const source of ["SECRET", "DO_NOT_EXPOSE", "ABCD-EFGH-IJKL-MNOP"]) {
        assert(!caught.message.includes(source));
      }
    } finally { caught.free(); }
  };
  const inactive = (session) => {
    error(() => session.push(new Uint8Array()), "NotActive");
    error(() => session.push(bytes("x")), "NotActive");
    error(() => session.finish(), "NotActive");
  };
  const explanation = (view) => {
    try {
      if (view.kind === mod.ExplanationKind.Classified) {
        return { kind: "Classified", detectionMode: name(mod.DetectionModeKind, view.detectionMode) };
      }
      assert.equal(view.kind, mod.ExplanationKind.Ambiguous);
      return { kind: "Ambiguous", candidateEvidence: name(mod.CandidateEvidenceKind, view.candidateEvidence) };
    } finally { view.free(); }
  };
  const project = (engine, result) => ({
    sourceBytes: result.sourceBytes, findingCount: result.findingCount(), candidateCount: result.candidateCount(),
    needsReview: result.needsReview(), hasCritical: result.hasCritical(),
    findings: Array.from({ length: result.findingCount() }, (_, i) => {
      const f = result.findingAt(i);
      try {
        return { ruleId: f.ruleId, start: f.start, end: f.end, line: f.line, column: f.column,
          severity: name(mod.FindingSeverity, f.severity), confidence: name(mod.FindingConfidence, f.confidence),
          remediation: f.remediation === mod.RemediationKind.None ? null : name(mod.RemediationKind, f.remediation),
          explanation: explanation(engine.explainFinding(result, i)) };
      } finally { f.free(); }
    }),
    candidates: Array.from({ length: result.candidateCount() }, (_, i) => {
      const c = result.candidateAt(i);
      try {
        return { kind: name(mod.CandidateKind, c.kind), evidence: name(mod.CandidateEvidenceKind, c.evidence),
          start: c.start, end: c.end, line: c.line, column: c.column,
          explanation: explanation(result.candidateExplanationAt(i)) };
      } finally { c.free(); }
    }),
  });

  let index = 0;
  let reports = 0;
  for (const spec of cases) {
    const engine = make(spec);
    for (const source of spec.sources) {
      const expected = oracle[index++];
      assert.equal(expected.name, spec.name);
      assert.equal(expected.source, source);
      const whole = engine.scan(source);
      assert.deepEqual(project(engine, whole), expected.report);
      if (expected.error) {
        error(() => engine.stream(), expected.error);
        error(() => engine.streamWithLimits({}), expected.error);
        whole.free();
        continue;
      }
      const input = bytes(source);
      for (let split = 0; split <= input.length; split++) {
        const session = engine.stream();
        for (const chunk of [input.subarray(0, split), new Uint8Array(), input.subarray(split)]) {
          const temporary = Uint8Array.from(chunk);
          session.push(temporary);
          temporary.fill(0xff); // the caller's buffer is never retained
        }
        const result = session.finish();
        inactive(session);
        session.free();
        assert.deepEqual(project(engine, result), expected.report, `${spec.name}: split ${split}`);
        result.free();
        reports++;
      }
      for (const size of [1, 2, 3, 7, 31]) {
        const session = engine.streamWithLimits({});
        for (let offset = 0; offset < input.length; offset += size) session.push(input.subarray(offset, offset + size));
        const result = session.finish();
        session.free();
        assert.deepEqual(project(engine, result), expected.report);
        result.free();
        reports++;
      }
      whole.free();
    }
    engine.free();
  }
  assert.equal(index, oracle.length);

  const engine = literal();
  const fields = ["maxCandidates", "maxStreamStorageBytes", "maxRetainedResults"];
  const invalid = [-1, -0.5, 0.5, NaN, Infinity, -Infinity, 2 ** 32, Number.MAX_SAFE_INTEGER,
    Number.MAX_SAFE_INTEGER + 1, 1e100, null, true, false, "1", 1n, {}, [], new Number(1)];
  for (const field of fields) {
    for (const value of invalid) error(() => engine.streamWithLimits({ [field]: value }), "InvalidLimit");
  }
  const builtin = new mod.ScanEngine();
  error(() => builtin.streamWithLimits({ maxCandidates: NaN }), "InvalidLimit");
  builtin.free();
  for (const opts of [{}, { maxCandidates: undefined, maxStreamStorageBytes: undefined, maxRetainedResults: undefined },
    { maxCandidates: 0xffff_ffff, maxStreamStorageBytes: 0xffff_ffff, maxRetainedResults: 0xffff_ffff }]) {
    const session = engine.streamWithLimits(opts);
    session.push(bytes("SECRET SECRET SECRET"));
    const result = session.finish();
    assert.equal(result.findingCount(), 3);
    result.free(); session.free();
  }
  error(() => engine.streamWithLimits({ maxStreamStorageBytes: 0 }), "StorageLimitExceeded");
  error(() => engine.streamWithLimits({ maxStreamStorageBytes: 1 }), "StorageLimitExceeded");
  error(() => engine.streamWithLimits({ maxCandidates: 0, maxRetainedResults: 0, maxStreamStorageBytes: 0 }), "StorageLimitExceeded");
  for (const [field, expected] of [["maxCandidates", "CandidateLimitExceeded"], ["maxRetainedResults", "ResultLimitExceeded"]]) {
    for (const value of [0, -0, 1]) {
      const session = engine.streamWithLimits({ [field]: value });
      if (value === 1) session.push(bytes("SECRET "));
      error(() => session.push(bytes("SECRET ")), expected);
      inactive(session); session.free();
    }
  }
  const reviewEngine = make({ rules: [] });
  const review = reviewEngine.streamWithLimits({ maxRetainedResults: 0 });
  review.push(bytes("ABCD-EFGH-IJKL-MNOP"));
  error(() => review.finish(), "ResultLimitExceeded"); inactive(review); review.free(); reviewEngine.free();
  const eofEngine = make({ rules: [{ id: "eof", kind: "pattern", value: "abc\\z", severity: "High" }] });
  const eof = eofEngine.streamWithLimits({ maxCandidates: 0 });
  eof.push(bytes("abc"));
  error(() => eof.finish(), "CandidateLimitExceeded"); inactive(eof); eof.free(); eofEngine.free();
  const suppressEngine = literal("ABCD-EFGH-IJKL-MNOP");
  const suppressed = suppressEngine.streamWithLimits({ maxRetainedResults: 1 });
  suppressed.push(bytes("ABCD-EFGH-IJKL-MNOP"));
  error(() => suppressed.finish(), "ResultLimitExceeded"); inactive(suppressed); suppressed.free(); suppressEngine.free();

  for (const malformed of [[0xff], [0xc0, 0xaf], [0xe2, 0x28, 0xa1], [0xed, 0xa0, 0x80]]) {
    const session = engine.stream();
    session.push(bytes("SECRET "));
    error(() => session.push(Uint8Array.from(malformed)), "InvalidUtf8"); inactive(session); session.free();
  }
  for (const incomplete of [[0xc3], [0xf0, 0x9f], [0xe2, 0x82]]) {
    const session = engine.stream();
    session.push(bytes("SECRET "));
    for (const byte of incomplete) session.push(Uint8Array.of(byte));
    error(() => session.finish(), "IncompleteUtf8"); inactive(session); session.free();
  }
  const malformedCarry = engine.stream();
  malformedCarry.push(Uint8Array.of(0xf0, 0x9f));
  error(() => malformedCarry.push(bytes("x")), "InvalidUtf8"); inactive(malformedCarry); malformedCarry.free();

  // The stream may outlive its engine, and results may outlive both. A separately
  // built compatible engine resolves explanations using existing Core validation.
  const owner = literal();
  const session = owner.stream();
  owner.free();
  const source = "é🙂 SECRET\nABCD-EFGH-IJKL-MNOP";
  session.push(bytes(source));
  const retained = session.finish();
  session.free();
  const expected = engine.scan(source);
  assert.deepEqual(project(engine, retained), project(engine, expected));
  const unrelated = make({ rules: [] });
  assert.throws(() => unrelated.explainFinding(retained, 0));
  unrelated.free();
  if (variant === "flow") {
    const key = new Uint8Array(32).fill(7);
    for (const [method, args] of [["redact", []], ["redactWith", ["<hidden>"]], ["template", []],
      ["templateWith", ["SAFE", true]], ["pseudonymize", [key, "pseudo_", 16]], ["synthesize", [key, "synthetic"]]]) {
      assert.equal(retained[method](source, ...args), expected[method](source, ...args));
    }
  }
  expected.free(); retained.free(); engine.free();
  console.log(`${variant}: streaming passed ${reports} direct-Core report comparisons; UTF-8, numeric validation, limits, terminal lifecycle and ownership`);
}
