import assert from "node:assert/strict";
import { before, test } from "node:test";

import {
  candidateSource,
  customRules,
  customScannerOptions,
  customSource,
  loadPackage,
  plainData,
} from "../helpers.mjs";
import {
  buildNativeEngine,
  loadNative,
  nativeScan,
  nativeStream,
} from "./native-oracle.mjs";

const encoder = new TextEncoder();

let api;
let native;

before(async () => {
  api = await loadPackage();
  await api.init();
  native = await loadNative();
});

function expectCribra(operation, code, domain) {
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
  assert.equal("cause" in caught, false, "cause is omitted");
  assert.equal("free" in caught, false, "no native handle on errors");
  return caught;
}

function assertNoNativeHandles(session) {
  assert.equal(Object.isFrozen(session), true);
  assert.equal("free" in session, false);
  assert.equal("__wbg_ptr" in session, false);
  assert.deepEqual(Object.keys(session).sort(), [
    "dispose",
    "disposed",
    "finish",
    "push",
  ]);
}

test("streams admitted custom rulesets across every chunk boundary", () => {
  const scanner = api.createScanner(customScannerOptions());
  const nativeEngine = buildNativeEngine(native, customRules);
  try {
    const source = customSource;
    const expected = scanner.scan(source);
    const nativeExpected = nativeScan(native, nativeEngine, source);
    assert.deepEqual(expected, nativeExpected);
    assert.equal(expected.findingCount, 4);

    const input = encoder.encode(source);
    for (let split = 0; split <= input.length; split += 1) {
      const session = scanner.stream();
      assertNoNativeHandles(session);
      assert.equal(session.disposed, false);

      const first = Uint8Array.from(input.subarray(0, split));
      session.push(first);
      first.fill(0xff); // The caller's buffer is never retained.
      session.push(new Uint8Array());
      session.push(input.subarray(split));

      const result = session.finish();
      plainData(result);
      assert.deepEqual(result, expected, `split ${split}`);
      assert.deepEqual(result, nativeExpected, `native split ${split}`);
      assert.equal(session.disposed, true);
      expectCribra(
        () => session.push(new Uint8Array()),
        "stream_not_active",
        "stream",
      );
      expectCribra(() => session.finish(), "stream_not_active", "stream");
      session.dispose();
      session.dispose();
    }

    for (const split of [0, 1, Math.floor(input.length / 2), input.length]) {
      assert.deepEqual(
        nativeStream(native, nativeEngine, [
          input.subarray(0, split),
          new Uint8Array(),
          input.subarray(split),
        ]),
        expected,
      );
    }
  } finally {
    nativeEngine.free();
    scanner.dispose();
  }
});

test("splits UTF-8 scalars and CRLF pairs without changing results", () => {
  const source = "前\r\n秘密🔑\r\nbeta";
  const rules = [
    { id: "emoji", kind: "literal", value: "秘密🔑", severity: "high" },
    { id: "beta", kind: "literal", value: "beta", severity: "low" },
  ];
  const scanner = api.createScanner({ builtins: "none", rules });
  try {
    const expected = scanner.scan(source);
    assert.equal(expected.findingCount, 2);
    const input = encoder.encode(source);
    const emojiStart = encoder.encode("前\r\n").length;
    const emojiEnd = emojiStart + encoder.encode("秘密🔑").length;
    const crlf = encoder.encode("前").length;

    const splits = new Set([0, input.length, crlf, crlf + 1]);
    for (let index = emojiStart; index <= emojiEnd; index += 1) {
      splits.add(index);
    }

    for (const split of splits) {
      const session = scanner.stream();
      session.push(input.subarray(0, split));
      session.push(input.subarray(split));
      assert.deepEqual(session.finish(), expected, `split ${split}`);
      session.dispose();
    }
  } finally {
    scanner.dispose();
  }
});

test("preserves finding order, candidates, and byte counts across chunks", () => {
  const source = "needle x needle y needle";
  const scanner = api.createScanner({
    builtins: "none",
    rules: [{ id: "needle", kind: "literal", value: "needle", severity: "high" }],
  });
  try {
    const expected = scanner.scan(source);
    assert.deepEqual(
      expected.findings.map((finding) => finding.span.start),
      [0, 9, 18],
    );
    const input = encoder.encode(source);
    for (let split = 0; split <= input.length; split += 3) {
      const session = scanner.stream();
      session.push(input.subarray(0, split));
      session.push(input.subarray(split));
      const result = session.finish();
      assert.deepEqual(result, expected, `split ${split}`);
      assert.equal(result.sourceBytes, input.length);
      session.dispose();
    }

    const candidateScanner = api.createScanner({ builtins: "none" });
    const candidateExpected = candidateScanner.scan(candidateSource);
    assert.equal(candidateExpected.candidateCount, 1);
    const candidateBytes = encoder.encode(candidateSource);
    const candidateSession = candidateScanner.stream();
    candidateSession.push(candidateBytes.subarray(0, 8));
    candidateSession.push(candidateBytes.subarray(8));
    assert.deepEqual(candidateSession.finish(), candidateExpected);
    candidateSession.dispose();
    candidateScanner.dispose();
  } finally {
    scanner.dispose();
  }
});

test("rejects non-streamable rulesets at admission without fallback", () => {
  const scanner = api.createScanner();
  try {
    const error = expectCribra(
      () => scanner.stream(),
      "stream_unsupported_ruleset",
      "stream",
    );
    assert.equal(error.terminal, undefined);
    assert.ok(scanner.scan("password=CorrectHorseBatteryStaple").findingCount > 0);
  } finally {
    scanner.dispose();
  }
});

test("terminal UTF-8 failures release the stream and preserve the scanner", () => {
  const scanner = api.createScanner({
    builtins: "none",
    rules: [{ id: "secret", kind: "literal", value: "SECRET", severity: "high" }],
  });
  try {
    const invalid = scanner.stream();
    invalid.push(encoder.encode("SECRET "));
    expectCribra(
      () => invalid.push(Uint8Array.of(0xff)),
      "stream_invalid_utf8",
      "stream",
    );
    assert.equal(invalid.disposed, true);
    expectCribra(
      () => invalid.push(new Uint8Array()),
      "stream_not_active",
      "stream",
    );
    expectCribra(() => invalid.finish(), "stream_not_active", "stream");

    const incomplete = scanner.stream();
    incomplete.push(encoder.encode("SECRET "));
    incomplete.push(Uint8Array.of(0xc3));
    const error = expectCribra(
      () => incomplete.finish(),
      "stream_incomplete_utf8",
      "stream",
    );
    assert.equal(JSON.stringify(error).includes("SECRET"), false);
    expectCribra(() => incomplete.finish(), "stream_not_active", "stream");

    assert.equal(scanner.scan("SECRET").findingCount, 1);
  } finally {
    scanner.dispose();
  }
});

test("validates stream limits before admission and enforces native budgets", () => {
  const scanner = api.createScanner({
    builtins: "none",
    rules: [{ id: "secret", kind: "literal", value: "SECRET", severity: "high" }],
  });
  try {
    const fields = [
      "maxCandidates",
      "maxStreamStorageBytes",
      "maxRetainedResults",
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
          () => scanner.stream({ limits: { [field]: value } }),
          "stream_invalid_limit",
          "stream",
        );
      }
    }
    expectCribra(() => scanner.stream(null), "invalid_argument", "stream");
    expectCribra(
      () => scanner.stream({ limits: null }),
      "invalid_argument",
      "stream",
    );

    expectCribra(
      () => scanner.stream({ limits: { maxStreamStorageBytes: 0 } }),
      "stream_storage_limit",
      "stream",
    );

    const candidates = scanner.stream({ limits: { maxCandidates: 0 } });
    expectCribra(
      () => candidates.push(encoder.encode("SECRET")),
      "stream_candidate_limit",
      "stream",
    );
    assert.equal(candidates.disposed, true);

    const results = scanner.stream({ limits: { maxRetainedResults: 0 } });
    expectCribra(
      () => results.push(encoder.encode("SECRET")),
      "stream_result_limit",
      "stream",
    );
    assert.equal(results.disposed, true);

    const reviewScanner = api.createScanner({ builtins: "none" });
    const review = reviewScanner.stream({ limits: { maxRetainedResults: 0 } });
    review.push(encoder.encode(candidateSource));
    expectCribra(() => review.finish(), "stream_result_limit", "stream");
    assert.equal(review.disposed, true);
    reviewScanner.dispose();

    // Negative zero is the adapter's accepted representation of zero.
    const zero = scanner.stream({
      limits: { maxCandidates: -0, maxRetainedResults: -0 },
    });
    assert.deepEqual(zero.finish(), scanner.scan(""));
    zero.dispose();

    assert.equal(scanner.scan("SECRET").findingCount, 1);
  } finally {
    scanner.dispose();
  }
});

test("distinguishes consumed, failed, and explicitly disposed streams", () => {
  const scanner = api.createScanner(customScannerOptions());
  try {
    const finished = scanner.stream();
    finished.push(encoder.encode(customSource));
    const result = finished.finish();
    assert.equal(finished.disposed, true);
    finished.dispose();
    expectCribra(
      () => finished.push(new Uint8Array()),
      "stream_not_active",
      "stream",
    );
    expectCribra(() => finished.finish(), "stream_not_active", "stream");
    assert.equal(result.findingCount, 4);
    assert.equal(result.findings[0].ruleId, "test.literal");

    const disposed = scanner.stream();
    disposed.push(encoder.encode("custom-secret-123"));
    disposed.dispose();
    assert.equal(disposed.disposed, true);
    assert.equal(disposed.dispose(), undefined);
    expectCribra(
      () => disposed.push(new Uint8Array()),
      "disposed",
      "stream",
    );
    expectCribra(() => disposed.finish(), "disposed", "stream");
    assert.equal(scanner.scan(customSource).findingCount, 4);
  } finally {
    scanner.dispose();
  }
});

test("sessions outlive their scanner and return detached results", () => {
  const scanner = api.createScanner(customScannerOptions());
  const session = scanner.stream();
  session.push(encoder.encode(customSource.slice(0, 10)));
  scanner.dispose();
  session.push(encoder.encode(customSource.slice(10)));
  const result = session.finish();
  session.dispose();
  assert.equal(result.findingCount, 4);
  plainData(result);
  assert.equal(JSON.stringify(result).includes(customSource), false);
  expectCribra(() => scanner.scan(customSource), "disposed", "scanner");

  const emptyScanner = api.createScanner({ builtins: "none" });
  try {
    const empty = emptyScanner.stream();
    assert.equal(empty.finish().sourceBytes, 0);
    empty.dispose();
    assert.equal(emptyScanner.scan("clean").findingCount, 0);
  } finally {
    emptyScanner.dispose();
  }
});
