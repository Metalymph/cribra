import assert from "node:assert/strict";
import { before, test } from "node:test";

import {
  customRules,
  customScannerOptions,
  customSource,
  loadPackage,
  plainData,
} from "../helpers.mjs";
import {
  buildNativeEngine,
  loadNative,
  nativeSnapshot,
} from "./native-oracle.mjs";

const encoder = new TextEncoder();
const byteLength = (source) => encoder.encode(source).length;
const byteOffset = (source, index) =>
  encoder.encode(source.slice(0, index)).length;

let api;
let native;

before(async () => {
  api = await loadPackage();
  await api.init();
  native = await loadNative();
});

function expectCribra(operation, code, domain, terminal) {
  let caught;
  try {
    operation();
  } catch (error) {
    caught = error;
  }
  assert.ok(caught, `expected CribraError(${code})`);
  assert.equal(caught.name, "CribraError");
  assert.equal(caught.code, code);
  if (domain !== undefined) assert.equal(caught.domain, domain);
  if (terminal !== undefined) assert.equal(caught.terminal, terminal);
  assert.equal("cause" in caught, false, "cause is omitted");
  assert.equal("free" in caught, false, "no native handle on errors");
  return caught;
}

function assertNoNativeHandles(session) {
  assert.equal(Object.isFrozen(session), true);
  assert.equal("free" in session, false);
  assert.equal("__wbg_ptr" in session, false);
  assert.deepEqual(Object.keys(session).sort(), [
    "applyEdit",
    "dispose",
    "disposed",
    "isActive",
    "snapshot",
  ]);
}

test("edits, revisions, and snapshots match fresh and native incremental scans", () => {
  const scanner = api.createScanner(customScannerOptions());
  const nativeEngine = buildNativeEngine(native, customRules);
  const nativeSession = nativeEngine.incremental(customSource);
  const session = scanner.incremental(customSource);
  try {
    let revision = 0n;
    let source = customSource;

    const check = () => {
      assert.equal(session.isActive, true);
      assert.equal(session.disposed, false);
      const snapshot = session.snapshot();
      assert.equal(snapshot.revision, revision);
      assert.deepEqual(snapshot.result, scanner.scan(source));
      assert.deepEqual(
        { revision: snapshot.revision, result: snapshot.result },
        nativeSnapshot(native, nativeSession),
      );
      plainData(snapshot);
      return snapshot;
    };

    check();

    const applyBoth = (startIndex, endIndex, replacement) => {
      const start = byteOffset(source, startIndex);
      const end = byteOffset(source, endIndex);
      const next = source.slice(0, startIndex) + replacement + source.slice(endIndex);
      const nativeRevision = nativeSession.applyEdit(
        revision,
        start,
        end,
        replacement,
      );
      const nextRevision = session.applyEdit(
        revision,
        start,
        end,
        replacement,
      );
      assert.equal(typeof nextRevision, "bigint");
      assert.equal(nextRevision, revision + 1n);
      assert.equal(nativeRevision, nextRevision);
      revision = nextRevision;
      source = next;
      check();
    };

    applyBoth(0, 0, "PREFIX ");
    const literalStart = source.indexOf("custom-secret-123");
    applyBoth(
      literalStart,
      literalStart + "custom-secret-123".length,
      "custom-secret-456",
    );
    const prefixStart = source.indexOf("PRE_beta\n");
    applyBoth(prefixStart, prefixStart + "PRE_beta\n".length, "");
    applyBoth(source.length, source.length, "");

    assert.equal(revision, 4n);
  } finally {
    session.dispose();
    nativeSession.free();
    nativeEngine.free();
    scanner.dispose();
  }
});

test("uses exact UTF-8 byte offsets for multibyte and supplementary edits", () => {
  const rules = [
    { id: "secret", kind: "literal", value: "秘密🔑", severity: "high" },
    { id: "accent", kind: "literal", value: "é🙂", severity: "low" },
  ];
  const scanner = api.createScanner({ builtins: "none", rules });
  const session = scanner.incremental("前\r\né🙂é 秘密🔑");
  try {
    let revision = 0n;
    let source = "前\r\né🙂é 秘密🔑";

    const apply = (startIndex, endIndex, replacement) => {
      const start = byteOffset(source, startIndex);
      const end = byteOffset(source, endIndex);
      const next =
        source.slice(0, startIndex) + replacement + source.slice(endIndex);
      revision = session.applyEdit(revision, start, end, replacement);
      source = next;
      assert.deepEqual(session.snapshot().result, scanner.scan(source));
    };

    apply(1, 1, "🦀");
    const secretStart = source.indexOf("秘密🔑");
    apply(secretStart, secretStart + "秘密🔑".length, "秘密");
    apply(source.length, source.length, "🦀");

    const before = session.snapshot();
    const emojiStart = byteOffset(source, source.indexOf("🦀"));
    expectCribra(
      () => session.applyEdit(revision, emojiStart + 1, emojiStart + 3, ""),
      "incremental_invalid_utf8_boundary",
      "incremental",
      false,
    );
    assert.equal(session.isActive, true);
    assert.equal(session.snapshot().revision, before.revision);
    assert.deepEqual(session.snapshot().result, before.result);

    expectCribra(
      () => session.applyEdit(revision, 4, 2, ""),
      "incremental_invalid_range",
      "incremental",
      false,
    );
    expectCribra(
      () => session.applyEdit(revision, byteLength(source) + 1, byteLength(source) + 1, ""),
      "incremental_invalid_range",
      "incremental",
      false,
    );
    expectCribra(
      () => session.applyEdit(revision, 1, 1, ""),
      "incremental_invalid_utf8_boundary",
      "incremental",
      false,
    );
  } finally {
    session.dispose();
    scanner.dispose();
  }
});

test("validates exact bigint revisions and native offset widths", () => {
  const scanner = api.createScanner({
    builtins: "none",
    rules: [{ id: "secret", kind: "literal", value: "SECRET", severity: "high" }],
  });
  const session = scanner.incremental("é🙂 SECRET");
  try {
    const original = session.snapshot();

    const invalidRevisions = [
      -1n,
      1n << 64n,
      (1n << 64n) + 1n,
      0,
      "0",
      null,
      undefined,
      NaN,
      1.5,
      {},
      true,
    ];
    for (const revision of invalidRevisions) {
      expectCribra(
        () => session.applyEdit(revision, 0, 0, "DO_NOT_EXPOSE"),
        "incremental_invalid_revision",
        "incremental",
        false,
      );
      assert.equal(session.isActive, true);
    }

    for (const revision of [1n, 9_007_199_254_740_993n, (1n << 64n) - 1n]) {
      expectCribra(
        () => session.applyEdit(revision, 0, 0, ""),
        "incremental_stale_revision",
        "incremental",
        false,
      );
    }

    const invalidOffsets = [
      -1,
      -0.5,
      0.5,
      NaN,
      Infinity,
      -Infinity,
      2 ** 32,
      Number.MAX_SAFE_INTEGER,
      Number.MAX_SAFE_INTEGER + 1,
      1e100,
      null,
      true,
      false,
      "0",
      1n,
      {},
      [],
      new Number(1),
    ];
    for (const value of invalidOffsets) {
      expectCribra(
        () => session.applyEdit(0n, value, 0, ""),
        "incremental_offset_out_of_range",
        "incremental",
        false,
      );
      expectCribra(
        () => session.applyEdit(0n, 0, value, ""),
        "incremental_offset_out_of_range",
        "incremental",
        false,
      );
      assert.equal(session.isActive, true);
    }

    expectCribra(
      () => session.applyEdit(0n, 0, 0, 42),
      "invalid_argument",
      "incremental",
      false,
    );

    assert.deepEqual(session.snapshot().result, original.result);
    assert.equal(session.snapshot().revision, original.revision);

    const revision = session.applyEdit(0n, -0, -0, "");
    assert.equal(typeof revision, "bigint");
    assert.equal(revision, 1n);
    assert.equal(session.snapshot().revision, 1n);
  } finally {
    session.dispose();
    scanner.dispose();
  }
});

test("terminal incremental failures preserve the last committed snapshot", () => {
  const scanner = api.createScanner({
    builtins: "none",
    rules: [{ id: "secret", kind: "literal", value: "SECRET", severity: "high" }],
  });
  const session = scanner.incremental("", { limits: { maxCandidates: 0 } });
  try {
    const initial = session.snapshot();
    assert.equal(initial.revision, 0n);
    assert.equal(initial.result.sourceBytes, 0);

    expectCribra(
      () => session.applyEdit(0n, 0, 0, "SECRET"),
      "incremental_candidate_limit",
      "incremental",
      true,
    );
    assert.equal(session.isActive, false);
    assert.equal(session.disposed, false);

    const terminal = session.snapshot();
    assert.equal(terminal.revision, 0n);
    assert.equal(terminal.result.sourceBytes, 0);
    assert.deepEqual(terminal.result, initial.result);
    assert.deepEqual(terminal.result, scanner.scan(""));

    expectCribra(
      () => session.applyEdit(0n, 0, 0, ""),
      "incremental_not_active",
      "incremental",
      true,
    );
    expectCribra(
      () => session.applyEdit(-1n, 0, 0, ""),
      "incremental_invalid_revision",
      "incremental",
      true,
    );
    expectCribra(
      () => session.applyEdit(0n, NaN, 0, ""),
      "incremental_offset_out_of_range",
      "incremental",
      true,
    );
    assert.deepEqual(session.snapshot().result, initial.result);

    session.dispose();
    assert.equal(session.disposed, true);
    assert.equal(session.isActive, false);
    expectCribra(() => session.snapshot(), "disposed", "incremental");
    expectCribra(
      () => session.applyEdit(0n, 0, 0, ""),
      "disposed",
      "incremental",
    );
    session.dispose();
  } finally {
    scanner.dispose();
  }
});

test("validates incremental limits and construction failures", () => {
  const scanner = api.createScanner({
    builtins: "none",
    rules: [{ id: "secret", kind: "literal", value: "SECRET", severity: "high" }],
  });
  try {
    const fields = [
      "maxCandidates",
      "maxRetainedCacheRecords",
      "maxMaterializedResults",
      "maxTransactionStorageBytes",
    ];
    const invalid = [
      -1,
      -0.5,
      0.5,
      NaN,
      Infinity,
      -Infinity,
      2 ** 32,
      Number.MAX_SAFE_INTEGER,
      Number.MAX_SAFE_INTEGER + 1,
      1e100,
      null,
      true,
      false,
      "1",
      1n,
      {},
      [],
      new Number(1),
    ];
    for (const field of fields) {
      for (const value of invalid) {
        expectCribra(
          () => scanner.incremental("SECRET", { limits: { [field]: value } }),
          "incremental_invalid_limit",
          "incremental",
          false,
        );
      }
    }
    expectCribra(
      () => scanner.incremental(42),
      "invalid_argument",
      "incremental",
      false,
    );
    expectCribra(
      () => scanner.incremental("SECRET", null),
      "invalid_argument",
      "incremental",
      false,
    );
    expectCribra(
      () => scanner.incremental("SECRET", { limits: null }),
      "invalid_argument",
      "incremental",
      false,
    );
    expectCribra(
      () => scanner.incremental("", { limits: { maxTransactionStorageBytes: 0 } }),
      "incremental_transaction_storage_limit",
      "incremental",
      false,
    );

    const zero = scanner.incremental("", { limits: { maxCandidates: 0 } });
    assert.equal(zero.applyEdit(0n, 0, 0, ""), 1n);
    assert.equal(zero.isActive, true);
    zero.dispose();

    const zeroes = Object.fromEntries(fields.map((field) => [field, 0]));
    zeroes.maxTransactionStorageBytes = undefined;
    const accepted = scanner.incremental("", { limits: zeroes });
    assert.equal(accepted.applyEdit(0n, 0, 0, ""), 1n);
    accepted.dispose();

    const max = Object.fromEntries(fields.map((field) => [field, 0xffff_ffff]));
    const bounded = scanner.incremental("SECRET", { limits: max });
    assert.equal(bounded.applyEdit(0n, 0, 0, ""), 1n);
    bounded.dispose();

    assert.equal(scanner.scan("SECRET").findingCount, 1);
  } finally {
    scanner.dispose();
  }
});

test("snapshots remain detached after edits and session disposal", () => {
  const scanner = api.createScanner(customScannerOptions());
  const session = scanner.incremental(customSource);
  try {
    const initial = session.snapshot();
    const initialResult = initial.result;

    const start = byteOffset(customSource, 0);
    const revision = session.applyEdit(0n, start, start, "ADDED ");
    assert.equal(revision, 1n);
    assert.equal(initial.revision, 0n);
    assert.deepEqual(initial.result, initialResult);
    assert.equal(initial.result.findingCount, 4);

    const later = session.snapshot();
    assert.equal(later.revision, 1n);
    assert.notStrictEqual(later.result, initial.result);

    session.dispose();
    assert.equal(initial.revision, 0n);
    assert.deepEqual(initial.result, initialResult);
    assert.equal(initial.result.findingCount, 4);
    assert.equal(later.result.findings.length, 4);
    assert.equal(scanner.scan(customSource).findingCount, 4);

    plainData(initial);
    plainData(later);
    assert.throws(() => JSON.stringify(initial), TypeError);
    assert.equal(JSON.parse(JSON.stringify(initial.result)).findingCount, 4);
    assert.equal(JSON.stringify(initial.result).includes(customSource), false);
  } finally {
    scanner.dispose();
  }
});

test("incremental sessions outlive their scanner with independent ownership", () => {
  const scanner = api.createScanner(customScannerOptions());
  const session = scanner.incremental(customSource);
  const initial = session.snapshot();
  try {
    scanner.dispose();
    expectCribra(() => scanner.scan(customSource), "disposed", "scanner");

    const start = byteOffset(customSource, customSource.length);
    assert.equal(session.applyEdit(0n, start, start, ""), 1n);
    const snapshot = session.snapshot();
    assert.equal(snapshot.revision, 1n);
    assert.equal(snapshot.result.findings.length, 4);
    assert.equal(initial.revision, 0n);

    const replacement = "DO_NOT_EXPOSE";
    const error = expectCribra(
      () => session.applyEdit(999n, 0, 0, replacement),
      "incremental_stale_revision",
      "incremental",
      false,
    );
    assert.equal(JSON.stringify(error).includes(replacement), false);
    assert.equal(JSON.stringify(snapshot.result).includes(customSource), false);
    assert.throws(() => JSON.stringify(snapshot), TypeError);

    session.dispose();
    session.dispose();
    assert.equal(initial.result.findingCount, 4);
    assert.equal(snapshot.result.findingCount, 4);
  } finally {
    scanner.dispose();
  }
});
