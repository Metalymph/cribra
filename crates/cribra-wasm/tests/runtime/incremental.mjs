// Actual generated Core/Flow incremental bindings against independent Core snapshots.
import assert from "node:assert/strict";
import { makeEngine, projectReport } from "./stream.mjs";

export function qualifyIncremental(mod, variant, cases, oracle) {
  const bytes = (source) => new TextEncoder().encode(source).length;
  const project = (engine, result) => projectReport(mod, engine, result);
  const error = (operation, code, terminal) => {
    let caught;
    try { operation(); } catch (value) { caught = value; }
    assert(caught instanceof mod.IncrementalError, `Expected IncrementalError.${code}`);
    try {
      assert.equal(caught.code, mod.IncrementalErrorCode[code]);
      assert.equal(caught.terminal, terminal);
      assert.equal(typeof caught.message, "string");
      assert(caught.message.length > 0);
      for (const privateText of ["SECRET", "DO_NOT_EXPOSE", "ABCD-EFGH-IJKL-MNOP", "CorrectHorseBatteryStaple"]) {
        assert(!caught.message.includes(privateText));
      }
    } finally { caught.free(); }
  };
  const transforms = (result, whole, source) => {
    if (variant !== "flow") return;
    const key = new Uint8Array(32).fill(9);
    const outcome = (operation) => {
      try { return { value: operation() }; }
      catch (error) {
        assert(error instanceof Error);
        return { error: error.message };
      }
    };
    for (const [method, args] of [["redact", []], ["redactWith", ["<hidden>"]], ["template", []],
      ["templateWith", ["SAFE", true]], ["pseudonymize", [key, "pseudo_", 16]], ["synthesize", [key, "synthetic"]]]) {
      // Core intentionally rejects some overlapping spans. Both successful
      // outputs and fail-closed transform outcomes must match whole-source scans.
      assert.deepEqual(outcome(() => result[method](source, ...args)), outcome(() => whole[method](source, ...args)));
    }
  };

  let snapshots = 0;
  let edits = 0;
  assert.equal(cases.length, oracle.length);
  for (let index = 0; index < cases.length; index++) {
    const spec = cases[index];
    const expected = oracle[index];
    assert.equal(expected.name, spec.name);
    const engine = makeEngine(mod, spec);
    const open = () => spec.limits === undefined ? engine.incremental(spec.source) : engine.incrementalWithLimits(spec.source, spec.limits);
    if (expected.constructionError) {
      error(open, expected.constructionError, false);
      engine.free();
      continue;
    }
    const session = open();
    const retained = [];
    const check = (state) => {
      assert.equal(session.isActive(), state.active);
      const snapshot = session.snapshot();
      assert.equal(typeof snapshot.revision, "bigint");
      assert.equal(snapshot.revision, BigInt(state.revision));
      const result = snapshot.result;
      const second = snapshot.result;
      assert(result instanceof mod.ScanResult);
      assert.notEqual(result, second);
      assert.deepEqual(project(engine, result), state.report, spec.name);
      assert.deepEqual(project(engine, second), state.report);
      const whole = engine.scan(state.source);
      assert.deepEqual(project(engine, result), project(engine, whole));
      transforms(result, whole, state.source);
      whole.free(); second.free();
      retained.push({ snapshot, result, state });
      snapshots++;
    };
    check(expected.states[0]);
    for (let i = 0; i < spec.edits.length; i++) {
      const edit = spec.edits[i];
      const state = expected.states[i + 1];
      const end = edit.end === "end" ? bytes(expected.states[i].source) : edit.end;
      const apply = () => session.applyEdit(BigInt(edit.revision), edit.start, end, edit.replacement);
      if (state.error) error(apply, state.error.code, state.error.terminal);
      else {
        const revision = apply();
        assert.equal(typeof revision, "bigint");
        assert.equal(revision, BigInt(state.revision));
      }
      check(state);
      edits++;
    }
    session.free();
    // Earlier snapshots survive every later edit/terminal failure and session free.
    for (const { snapshot, result, state } of retained) {
      assert.equal(snapshot.revision, BigInt(state.revision));
      const another = snapshot.result;
      snapshot.free();
      assert.deepEqual(project(engine, result), state.report);
      assert.deepEqual(project(engine, another), state.report);
      const whole = engine.scan(state.source);
      transforms(result, whole, state.source);
      transforms(another, whole, state.source);
      whole.free(); result.free(); another.free();
    }
    engine.free();
  }

  const rule = { rules: [{ kind: "literal", id: "literal", value: "SECRET", severity: "High" }] };
  const engine = makeEngine(mod, rule);
  const fields = ["maxCandidates", "maxRetainedCacheRecords", "maxMaterializedResults", "maxTransactionStorageBytes"];
  const invalid = [-1, -0.5, 0.5, NaN, Infinity, -Infinity, 2 ** 32, Number.MAX_SAFE_INTEGER,
    Number.MAX_SAFE_INTEGER + 1, 1e100, null, true, false, "1", 1n, {}, [], new Number(1)];
  for (const field of fields) {
    for (const value of invalid) error(() => engine.incrementalWithLimits("SECRET", { [field]: value }), "InvalidLimit", false);
  }
  for (const limits of [{}, Object.fromEntries(fields.map((field) => [field, undefined])),
    Object.fromEntries(fields.map((field) => [field, 0xffff_ffff]))]) {
    const session = engine.incrementalWithLimits("SECRET", limits);
    assert.equal(session.applyEdit(0n, 0, 0, ""), 1n);
    session.free();
  }
  for (const field of fields.slice(0, 3)) {
    for (const value of [0, -0]) {
      const session = engine.incrementalWithLimits("", { [field]: value });
      assert.equal(session.applyEdit(0n, 0, 0, ""), 1n);
      assert.equal(session.isActive(), true);
      session.free();
    }
  }
  error(() => engine.incrementalWithLimits("", { maxTransactionStorageBytes: -0 }), "TransactionStorageLimitExceeded", false);
  // All options validate before Core construction, even if a different zero budget fails.
  error(() => engine.incrementalWithLimits("SECRET", { maxCandidates: 0, maxTransactionStorageBytes: NaN }), "InvalidLimit", false);
  const session = engine.incremental("é🙂 SECRET");
  for (const value of invalid) {
    error(() => session.applyEdit(0n, value, 0, "DO_NOT_EXPOSE"), "OffsetOutOfRange", false);
    error(() => session.applyEdit(0n, 0, value, "DO_NOT_EXPOSE"), "OffsetOutOfRange", false);
    assert.equal(session.isActive(), true);
  }
  // Values in the target range reach Core unchanged; input errors preserve revision.
  error(() => session.applyEdit(0n, 0xffff_ffff, 0xffff_ffff, ""), "InvalidRange", false);
  for (const revision of [-1n, 1n << 64n, (1n << 64n) + 1n, 0, "0", null, undefined, NaN, {}, 1.5]) {
    error(() => session.applyEdit(revision, 0, 0, "DO_NOT_EXPOSE"), "InvalidRevision", false);
  }
  for (const revision of [1n, 9_007_199_254_740_993n, (1n << 64n) - 1n]) {
    error(() => session.applyEdit(revision, 0, 0, ""), "StaleRevision", false);
  }
  assert.equal(session.applyEdit(0n, -0, 0, ""), 1n);
  const fresh = session.snapshot();
  assert.equal(fresh.revision, 1n);
  assert.equal(session.applyEdit(fresh.revision, 0, 2, "前"), 2n);
  fresh.free(); session.free();

  const terminal = engine.incrementalWithLimits("", { maxCandidates: 0 });
  error(() => terminal.applyEdit(0n, 0, 0, "SECRET"), "CandidateLimitExceeded", true);
  error(() => terminal.applyEdit(0n, NaN, 0, ""), "OffsetOutOfRange", true);
  error(() => terminal.applyEdit(-1n, 0, 0, ""), "InvalidRevision", true);
  error(() => terminal.applyEdit(0n, 0, 0, ""), "NotActive", true);
  const last = terminal.snapshot();
  terminal.free();
  const lastResult = last.result;
  last.free();
  assert.equal(lastResult.sourceBytes, 0);
  lastResult.free();

  const owner = makeEngine(mod, rule);
  const independent = owner.incremental("SECRET");
  owner.free();
  assert.equal(independent.applyEdit(0n, 6, 6, " ABCD-EFGH-IJKL-MNOP"), 1n);
  const snapshot = independent.snapshot();
  independent.free();
  const result = snapshot.result;
  snapshot.free();
  assert.equal(result.findingCount(), 1);
  assert.equal(result.candidateCount(), 1);
  const explanation = engine.explainFinding(result, 0);
  explanation.free();
  const unrelated = makeEngine(mod, { rules: [] });
  assert.throws(() => unrelated.explainFinding(result, 0));
  unrelated.free(); result.free(); engine.free();
  assert.equal("source" in mod.IncrementalSession.prototype, false);
  console.log(`${variant}: incremental passed ${snapshots} direct-Core snapshots across ${edits} edits; numeric/bigint validation, four limits, terminal snapshots and ownership`);
}
