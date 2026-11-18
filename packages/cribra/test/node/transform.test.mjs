import assert from "node:assert/strict";
import { before, test } from "node:test";

import {
  customRules,
  customScannerOptions,
  customSource,
  loadPackage,
  occurrenceSpan,
  plainData,
  privacySentinel,
  profile,
} from "../helpers.mjs";
import {
  buildNativeEngine,
  loadNative,
  nativeTransform,
} from "./native-oracle.mjs";

const flow = profile === "flow";
const key = new Uint8Array(32).fill(7);

let api;
let native;
let scanner;
let nativeEngine;

before(async () => {
  api = await loadPackage();
  await api.init();
  native = await loadNative();
  scanner = api.createScanner(customScannerOptions());
  nativeEngine = buildNativeEngine(native, customRules);
});

function expectCribra(error, code, domain) {
  assert.equal(error.name, "CribraError");
  assert.equal(error.code, code);
  if (domain !== undefined) assert.equal(error.domain, domain);
  assert.equal("cause" in error, false, "cause is omitted");
  assert.equal("free" in error, false, "no native handle on errors");
  return true;
}

function requests() {
  return [
    { kind: "redact" },
    { kind: "redact", replacement: "<hidden>" },
    { kind: "template" },
    { kind: "template", namespace: "SAFE", numbered: true },
    { kind: "pseudonymize", key },
    { kind: "pseudonymize", key, prefix: "pseudo:", digestBytes: 8 },
    { kind: "synthesize", key },
    { kind: "synthesize", key, marker: "fixture" },
  ];
}

test("Flow transformations match direct native operations", { skip: !flow }, () => {
  const sources = [
    customSource,
    "clean text with é🦀",
    "é🦀 custom-secret-123 — token-1234",
  ];
  for (const source of sources) {
    for (const request of requests()) {
      const expected = nativeTransform(native, nativeEngine, source, request);
      const actual = scanner.transform(source, request);
      assert.deepEqual(actual.scan, scanner.scan(source), "scan DTO matches");
      if (expected.errorCode !== undefined) {
        assert.throws(
          () => scanner.transform(source, request),
          (error) => expectCribra(error, expected.errorCode, "transform"),
        );
        continue;
      }
      assert.equal(actual.output, expected.output);
      plainData(actual);
      assert.equal(Object.isFrozen(actual), true);
      assert.equal(Object.isFrozen(actual.scan), true);
      assert.equal(Object.isFrozen(actual.scan.findings), true);
    }
  }
});

test("transformations handle empty findings without rewriting source", { skip: !flow }, () => {
  const source = "ordinary é🦀 text";
  for (const request of requests()) {
    const result = scanner.transform(source, request);
    assert.equal(result.output, source);
    assert.equal(result.scan.findingCount, 0);
  }
});

test("unicode and supplementary findings preserve surrounding text", { skip: !flow }, () => {
  const source = "prefix🦀custom-secret-123suffix";
  const range = occurrenceSpan(source, "custom-secret-123");
  const result = scanner.transform(source, { kind: "redact" });
  assert.equal(result.output, "prefix🦀[REDACTED]suffix");
  assert.deepEqual(result.scan.findings[0].span, range);
  const template = scanner.transform(source, {
    kind: "template",
    namespace: "SAFE",
    numbered: true,
  });
  assert.match(template.output, /^prefix🦀<SAFE:/u);
  assert.match(template.output, />suffix$/u);
  assert.equal(template.output.includes("custom-secret-123"), false);
});

test("one native scan backs each transformation call", { skip: !flow }, () => {
  const original = native.ScanEngine.prototype.scan;
  let calls = 0;
  native.ScanEngine.prototype.scan = function patched(...args) {
    calls += 1;
    return original.apply(this, args);
  };
  try {
    for (const request of requests()) {
      calls = 0;
      const result = scanner.transform(customSource, request);
      assert.equal(calls, 1, `exactly one scan for ${request.kind}`);
      assert.equal(result.scan.findingCount, 4);
    }
  } finally {
    native.ScanEngine.prototype.scan = original;
  }
});

test("keyed transformations are deterministic for the same key", { skip: !flow }, () => {
  for (const kind of ["pseudonymize", "synthesize"]) {
    const first = scanner.transform("TOKEN=custom-secret-123", {
      kind,
      key,
    });
    const second = scanner.transform("TOKEN=custom-secret-123", {
      kind,
      key,
    });
    assert.equal(first.output, second.output);
    assert.equal(first.output.includes("custom-secret-123"), false);
    const other = scanner.transform("TOKEN=custom-secret-123", {
      kind,
      key: new Uint8Array(32).fill(8),
    });
    assert.notEqual(first.output, other.output);
  }
});

test("overlapping findings fail closed with a typed error", { skip: !flow }, () => {
  const overlapRules = [
    { id: "overlap.long", kind: "pattern", pattern: "SECRET123", severity: "high" },
    { id: "overlap.short", kind: "pattern", pattern: "SECRET", severity: "high" },
  ];
  const overlapScanner = api.createScanner({
    builtins: "none",
    rules: overlapRules,
  });
  const overlapEngine = buildNativeEngine(native, overlapRules);
  try {
    const source = "SECRET123";
    assert.equal(overlapScanner.scan(source).findingCount, 2);
    const expected = nativeTransform(native, overlapEngine, source, {
      kind: "template",
    });
    assert.equal(expected.errorCode, "transform_overlapping_spans");
    assert.throws(
      () => overlapScanner.transform(source, { kind: "template" }),
      (error) => expectCribra(error, "transform_overlapping_spans", "transform"),
    );
    const redacted = overlapScanner.transform(source, { kind: "redact" });
    assert.equal(redacted.output, "[REDACTED]");
  } finally {
    overlapScanner.dispose();
    overlapEngine.free();
  }
});

test("invalid requests fail before scanning and stay recoverable", { skip: !flow }, () => {
  const original = native.ScanEngine.prototype.scan;
  let calls = 0;
  native.ScanEngine.prototype.scan = function patched(...args) {
    calls += 1;
    return original.apply(this, args);
  };
  const invalid = [
    [null, "invalid_argument"],
    [undefined, "invalid_argument"],
    ["redact", "invalid_argument"],
    [1, "invalid_argument"],
    [[], "invalid_argument"],
    [{}, "invalid_argument"],
    [{ kind: "bogus" }, "invalid_argument"],
    [{ kind: "redact", replacement: 1 }, "invalid_argument"],
    [{ kind: "template", namespace: 1 }, "invalid_argument"],
    [{ kind: "template", numbered: "yes" }, "invalid_argument"],
    [{ kind: "pseudonymize" }, "invalid_key_length"],
    [{ kind: "pseudonymize", key: new Uint8Array(31) }, "invalid_key_length"],
    [{ kind: "pseudonymize", key: new Uint8Array(33) }, "invalid_key_length"],
    [{ kind: "pseudonymize", key: "not a key" }, "invalid_key_length"],
    [{ kind: "pseudonymize", key, prefix: 1 }, "invalid_argument"],
    [{ kind: "pseudonymize", key, digestBytes: -1 }, "invalid_argument"],
    [{ kind: "pseudonymize", key, digestBytes: 1.5 }, "invalid_argument"],
    [{ kind: "pseudonymize", key, digestBytes: "16" }, "invalid_argument"],
    [{ kind: "synthesize" }, "invalid_key_length"],
    [{ kind: "synthesize", key: new Uint8Array(31) }, "invalid_key_length"],
    [{ kind: "synthesize", key, marker: 1 }, "invalid_argument"],
  ];
  try {
    for (const [request, code] of invalid) {
      calls = 0;
      assert.throws(
        () => scanner.transform("token-1234", request),
        (error) => expectCribra(error, code, "transform"),
      );
      assert.equal(calls, 0, "invalid requests never scan");
    }
    assert.throws(
      () => scanner.transform(123, { kind: "redact" }),
      (error) => expectCribra(error, "invalid_argument", "transform"),
    );
    assert.equal(calls, 0);
  } finally {
    native.ScanEngine.prototype.scan = original;
  }
  assert.equal(
    scanner.transform("token-1234", { kind: "redact" }).output,
    "[REDACTED]",
  );
});

test("transform errors never expose source, matcher or key material", { skip: !flow }, () => {
  const secretScanner = api.createScanner({
    builtins: "none",
    rules: [
      {
        id: "privacy.literal",
        kind: "literal",
        value: privacySentinel,
        severity: "high",
      },
    ],
  });
  try {
    const source = `before ${privacySentinel} after`;
    const keyBytes = new Uint8Array(32).fill(9);
    const failures = [
      () => secretScanner.transform(source, { kind: "pseudonymize", key: new Uint8Array(31) }),
      () => secretScanner.transform(source, { kind: "bogus" }),
      () => secretScanner.transform(source, { kind: "synthesize", key: keyBytes, marker: 7 }),
    ];
    for (const failure of failures) {
      assert.throws(failure, (error) => {
        expectCribra(
          error,
          error.code === "invalid_key_length"
            ? "invalid_key_length"
            : "invalid_argument",
          "transform",
        );
        const surfaces = [
          error.message,
          String(error),
          JSON.stringify(error),
          String(error.stack ?? ""),
        ];
        for (const surface of surfaces) {
          assert.equal(surface.includes(privacySentinel), false);
          assert.equal(surface.includes(source), false);
          assert.equal(surface.includes("before "), false);
        }
        return true;
      });
    }
    const output = secretScanner.transform(source, { kind: "redact" }).output;
    assert.equal(output.includes(privacySentinel), false);
    assert.equal(output, "before [REDACTED] after");
  } finally {
    secretScanner.dispose();
  }
});

test("each transformation frees its native result exactly once", { skip: !flow }, () => {
  const disposable = api.createScanner(customScannerOptions());
  const overlapRules = [
    { id: "overlap.long", kind: "pattern", pattern: "SECRET123", severity: "high" },
    { id: "overlap.short", kind: "pattern", pattern: "SECRET", severity: "high" },
  ];
  const overlapScanner = api.createScanner({ builtins: "none", rules: overlapRules });
  const original = native.ScanResult.prototype.free;
  let frees = 0;
  native.ScanResult.prototype.free = function countedFree(...args) {
    frees += 1;
    return original.apply(this, args);
  };
  try {
    disposable.transform(customSource, { kind: "redact" });
    assert.equal(frees, 1, "successful transform frees once");

    frees = 0;
    assert.throws(
      () => overlapScanner.transform("SECRET123", { kind: "template" }),
      (error) => expectCribra(error, "transform_overlapping_spans", "transform"),
    );
    assert.equal(frees, 1, "failed transform still frees its scan");
  } finally {
    native.ScanResult.prototype.free = original;
    disposable.dispose();
    overlapScanner.dispose();
  }
});

test("transform results survive scanner disposal", { skip: !flow }, () => {
  const disposable = api.createScanner(customScannerOptions());
  const result = disposable.transform(customSource, {
    kind: "pseudonymize",
    key,
  });
  assert.equal(result.scan.findingCount, 4);
  disposable.dispose();
  assert.equal(result.scan.findingCount, 4);
  assert.equal(result.output.includes("custom-secret-123"), false);
  plainData(result);
});

test("Core rejects transformations without scanning or native fallback", { skip: flow }, () => {
  assert.equal(api.getCapabilities().profile, "core");
  assert.equal(api.getCapabilities().redaction, false);
  assert.equal(api.getCapabilities().templating, false);
  assert.equal(api.getCapabilities().pseudonymization, false);
  assert.equal(api.getCapabilities().synthesis, false);

  const original = native.ScanEngine.prototype.scan;
  let calls = 0;
  native.ScanEngine.prototype.scan = function patched(...args) {
    calls += 1;
    return original.apply(this, args);
  };
  try {
    for (const request of requests()) {
      calls = 0;
      assert.throws(
        () => scanner.transform(customSource, request),
        (error) => expectCribra(error, "unsupported_capability", "capability"),
      );
      assert.equal(calls, 0);
    }
  } finally {
    native.ScanEngine.prototype.scan = original;
  }
  assert.throws(
    () => scanner.transform("x", { kind: "bogus" }),
    (error) => expectCribra(error, "invalid_argument", "transform"),
  );
});
