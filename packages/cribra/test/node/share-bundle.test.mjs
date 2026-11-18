import assert from "node:assert/strict";
import { before, test } from "node:test";

import {
  customRules,
  customScannerOptions,
  loadPackage,
  plainData,
  privacySentinel,
  profile,
} from "../helpers.mjs";
import { loadNative, nativeShareBundle } from "./native-oracle.mjs";

const flow = profile === "flow";
const key = new Uint8Array(32).fill(7);
const explicitMillis = 1_700_000_000_123;

let api;
let native;
let scanner;

before(async () => {
  api = await loadPackage();
  await api.init();
  native = await loadNative();
  scanner = api.createScanner(customScannerOptions());
});

function expectCribra(error, code, domain) {
  assert.equal(error.name, "CribraError");
  assert.equal(error.code, code);
  if (domain !== undefined) assert.equal(error.domain, domain);
  assert.equal("cause" in error, false, "cause is omitted");
  assert.equal("free" in error, false, "no native handle on errors");
  return true;
}

function modes() {
  return [
    { kind: "redact" },
    { kind: "template" },
    { kind: "pseudonymize", key },
    { kind: "pseudonymize", key, prefix: "pseudo:", digestBytes: 8 },
    { kind: "synthesize", key },
    { kind: "synthesize", key, marker: "fixture" },
  ];
}

test("Flow Share Bundles match direct native construction", { skip: !flow }, () => {
  const collections = [
    [],
    [{ id: "clean", source: "ordinary é🦀 text" }],
    [
      { id: "same", source: "first custom-secret-123" },
      { id: "", source: "clean" },
      { id: "same", source: "🦀 custom-secret-123\nPRE_beta" },
      { id: "独立/🦀", source: "前🦀 token-1234" },
    ],
  ];
  for (const sources of collections) {
    for (const mode of modes()) {
      const expected = nativeShareBundle(
        native,
        customRules,
        sources,
        mode,
        explicitMillis,
      );
      const actual = api.createShareBundle(scanner, sources, {
        mode,
        generatedAt: new Date(explicitMillis),
      });
      assert.deepEqual(actual, expected);
      plainData(actual);
      assert.equal(Object.isFrozen(actual), true);
      assert.equal(Object.isFrozen(actual.entries), true);
      assert.equal(Object.isFrozen(actual.manifest), true);
      assert.equal(Object.isFrozen(actual.summary), true);
      for (const entry of actual.entries) {
        assert.equal(Object.isFrozen(entry), true);
      }
    }
  }
});

test("manifest, summary and entry order preserve input semantics", { skip: !flow }, () => {
  const sources = [
    { id: "b", source: "custom-secret-123" },
    { id: "a", source: "clean" },
    { id: "b", source: "PRE_beta" },
  ];
  const bundle = api.createShareBundle(scanner, sources, {
    mode: { kind: "redact" },
    generatedAt: new Date(explicitMillis),
  });
  assert.deepEqual(
    bundle.entries.map((entry) => entry.id),
    ["b", "a", "b"],
  );
  assert.deepEqual(
    bundle.entries.map((entry) => entry.content),
    ["[REDACTED]", "clean", "[REDACTED]"],
  );
  assert.equal(bundle.manifest.mode, "redact");
  assert.equal(bundle.manifest.generatedAtSeconds, 1_700_000_000);
  assert.equal(bundle.manifest.generatedAtNanoseconds, 123_000_000);
  assert.deepEqual(bundle.manifest.summary, bundle.summary);
  assert.equal(bundle.summary.scannedSources, 3);
  assert.equal(bundle.summary.totalFindings, 2);
  assert.equal(bundle.summary.reportsWithFindings, 2);
  assert.equal(bundle.summary.isClean, false);
});

test("empty bundles are valid and have clean zero summaries", { skip: !flow }, () => {
  const bundle = api.createShareBundle(scanner, [], {
    mode: { kind: "template" },
    generatedAt: new Date(explicitMillis),
  });
  assert.deepEqual(bundle.entries, []);
  assert.equal(bundle.manifest.mode, "template");
  assert.equal(bundle.summary.scannedSources, 0);
  assert.equal(bundle.summary.isClean, true);
});

test("the default timestamp comes from the backend host clock", { skip: !flow }, () => {
  const before = Date.now();
  const bundle = api.createShareBundle(scanner, [{ id: "x", source: "clean" }], {
    mode: { kind: "redact" },
  });
  const after = Date.now();
  const manifest = bundle.manifest;
  const millis =
    manifest.generatedAtSeconds * 1000 +
    manifest.generatedAtNanoseconds / 1e6;
  assert.equal(Number.isSafeInteger(manifest.generatedAtSeconds), true);
  assert(manifest.generatedAtNanoseconds >= 0);
  assert(manifest.generatedAtNanoseconds < 1_000_000_000);
  assert(millis >= before - 1000 && millis <= after + 1000);
});

test("invalid Share Bundle requests are structured and recoverable", { skip: !flow }, () => {
  const invalid = [
    [undefined, "invalid_argument"],
    [null, "invalid_argument"],
    [{}, "invalid_argument"],
    [{ mode: null }, "invalid_argument"],
    [{ mode: {} }, "invalid_argument"],
    [{ mode: { kind: "bogus" } }, "invalid_argument"],
    [{ mode: { kind: "redact" }, generatedAt: "now" }, "invalid_argument"],
    [{ mode: { kind: "redact" }, generatedAt: new Date(NaN) }, "share_timestamp_invalid"],
    [{ mode: { kind: "redact" }, generatedAt: new Date(-1) }, "share_timestamp_invalid"],
    [{ mode: { kind: "pseudonymize" } }, "invalid_key_length"],
    [{ mode: { kind: "pseudonymize", key: new Uint8Array(31) } }, "invalid_key_length"],
    [{ mode: { kind: "pseudonymize", key: new Uint8Array(33) } }, "invalid_key_length"],
    [{ mode: { kind: "pseudonymize", key: "not a key" } }, "invalid_key_length"],
    [{ mode: { kind: "pseudonymize", key, prefix: 1 } }, "invalid_argument"],
    [{ mode: { kind: "pseudonymize", key, digestBytes: -1 } }, "invalid_argument"],
    [{ mode: { kind: "pseudonymize", key, digestBytes: 1.5 } }, "invalid_argument"],
    [{ mode: { kind: "synthesize" } }, "invalid_key_length"],
    [{ mode: { kind: "synthesize", key: new Uint8Array(31) } }, "invalid_key_length"],
    [{ mode: { kind: "synthesize", key, marker: 1 } }, "invalid_argument"],
  ];
  for (const [options, code] of invalid) {
    assert.throws(
      () => api.createShareBundle(scanner, [], options),
      (error) => expectCribra(error, code, "share"),
    );
  }

  for (const sources of [
    "not an array",
    null,
    [null],
    [1],
    [{}],
    [{ id: "x" }],
    [{ source: "x" }],
    [{ id: 1, source: "x" }],
    [{ id: "x", source: 1 }],
  ]) {
    assert.throws(
      () => api.createShareBundle(scanner, sources, { mode: { kind: "redact" } }),
      (error) => expectCribra(error, "invalid_argument", "share"),
    );
  }

  const recovered = api.createShareBundle(scanner, [{ id: "x", source: "clean" }], {
    mode: { kind: "redact" },
    generatedAt: new Date(explicitMillis),
  });
  assert.equal(recovered.entries[0].id, "x");
});

test("Share Bundle failures and entries never expose source secrets", { skip: !flow }, () => {
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
    const bundle = api.createShareBundle(
      secretScanner,
      [{ id: "privacy.txt", source }],
      { mode: { kind: "redact" }, generatedAt: new Date(explicitMillis) },
    );
    assert.equal(bundle.entries[0].content, "before [REDACTED] after");
    assert.equal(JSON.stringify(bundle).includes(privacySentinel), false);
    assert.equal(JSON.stringify(bundle).includes(source), false);
    plainData(bundle);

    const keyBytes = new Uint8Array(32).fill(9);
    let caught;
    try {
      api.createShareBundle(secretScanner, [{ id: "x", source }], {
        mode: { kind: "pseudonymize", key: new Uint8Array(31) },
      });
    } catch (error) {
      caught = error;
    }
    assert(caught);
    expectCribra(caught, "invalid_key_length", "share");
    assert.equal(caught.message.includes(privacySentinel), false);
    assert.equal(caught.message.includes(source), false);
    assert.equal(JSON.stringify(caught).includes(String(keyBytes)), false);
  } finally {
    secretScanner.dispose();
  }
});

test("Share Bundle DTOs are detached and survive scanner disposal", { skip: !flow }, () => {
  const disposable = api.createScanner(customScannerOptions());
  const bundle = api.createShareBundle(
    disposable,
    [{ id: "kept", source: "custom-secret-123" }],
    { mode: { kind: "redact" }, generatedAt: new Date(explicitMillis) },
  );
  disposable.dispose();
  assert.equal(bundle.entries[0].id, "kept");
  assert.equal(bundle.entries[0].content, "[REDACTED]");
  assert.equal(bundle.manifest.mode, "redact");
  assert.equal(bundle.summary.totalFindings, 1);
  plainData(bundle);
  assert.equal(JSON.parse(JSON.stringify(bundle)).entries[0].id, "kept");
});

test("native builder, bundle, entries, manifest and summaries are freed exactly once", { skip: !flow }, () => {
  const disposable = api.createScanner(customScannerOptions());
  const sources = [
    { id: "a", source: "custom-secret-123" },
    { id: "b", source: "clean" },
  ];
  const counts = new Map();
  const prototypes = [
    ["builder", native.ShareBundleBuilder.prototype],
    ["bundle", native.ShareBundle.prototype],
    ["entry", native.ShareBundleEntry.prototype],
    ["manifest", native.ShareManifestView.prototype],
    ["summary", native.ScanSummaryView.prototype],
  ];
  const originals = new Map();
  for (const [name, prototype] of prototypes) {
    const original = prototype.free;
    originals.set(name, original);
    prototype.free = function countedFree(...args) {
      counts.set(name, (counts.get(name) ?? 0) + 1);
      return original.apply(this, args);
    };
  }
  try {
    api.createShareBundle(disposable, sources, {
      mode: { kind: "redact" },
      generatedAt: new Date(explicitMillis),
    });
  } finally {
    for (const [name, prototype] of prototypes) {
      prototype.free = originals.get(name);
    }
    disposable.dispose();
  }
  assert.equal(counts.get("builder"), 1, "builder freed once");
  assert.equal(counts.get("bundle"), 1, "bundle freed once");
  assert.equal(counts.get("entry"), sources.length, "each entry freed once");
  assert.equal(counts.get("manifest"), 1, "manifest freed once");
  assert.equal(counts.get("summary"), 2, "manifest and bundle summaries freed once");
});

test("a Core Share Bundle failure still frees the native builder", { skip: !flow }, () => {
  const overlapRules = [
    { id: "overlap.long", kind: "pattern", pattern: "SECRET123", severity: "high" },
    { id: "overlap.short", kind: "pattern", pattern: "SECRET", severity: "high" },
  ];
  const overlapScanner = api.createScanner({ builtins: "none", rules: overlapRules });
  const original = native.ShareBundleBuilder.prototype.free;
  let frees = 0;
  native.ShareBundleBuilder.prototype.free = function countedFree(...args) {
    frees += 1;
    return original.apply(this, args);
  };
  try {
    assert.throws(
      () => api.createShareBundle(
        overlapScanner,
        [{ id: "overlap", source: "SECRET123" }],
        { mode: { kind: "template" }, generatedAt: new Date(explicitMillis) },
      ),
      (error) => expectCribra(error, "share_unknown", "share"),
    );
    assert.equal(frees, 1, "builder freed on native failure");
  } finally {
    native.ShareBundleBuilder.prototype.free = original;
    overlapScanner.dispose();
  }
});

test("disposed scanners are rejected before any native Share Bundle work", { skip: !flow }, () => {
  const disposable = api.createScanner(customScannerOptions());
  disposable.dispose();

  const originalScan = native.ShareBundleBuilder.prototype.scanAndBuild;
  const originalGenerated = native.ShareBundleBuilder.prototype.scanAndBuildWithGeneratedAt;
  const originalRedact = native.ShareBundleBuilder.prototype.modeRedact;
  let calls = 0;
  native.ShareBundleBuilder.prototype.scanAndBuild = function patched(...args) {
    calls += 1;
    return originalScan.apply(this, args);
  };
  native.ShareBundleBuilder.prototype.scanAndBuildWithGeneratedAt = function patched(...args) {
    calls += 1;
    return originalGenerated.apply(this, args);
  };
  native.ShareBundleBuilder.prototype.modeRedact = function patched(...args) {
    calls += 1;
    return originalRedact.apply(this, args);
  };
  try {
    assert.throws(
      () => api.createShareBundle(disposable, [{ id: "x", source: "clean" }], {
        mode: { kind: "redact" },
      }),
      (error) => expectCribra(error, "disposed", "scanner"),
    );
    assert.equal(calls, 0);
  } finally {
    native.ShareBundleBuilder.prototype.scanAndBuild = originalScan;
    native.ShareBundleBuilder.prototype.scanAndBuildWithGeneratedAt = originalGenerated;
    native.ShareBundleBuilder.prototype.modeRedact = originalRedact;
  }
});

test("rejects foreign scanner objects instead of touching native state", { skip: !flow }, () => {
  assert.throws(
    () => api.createShareBundle({}, [], { mode: { kind: "redact" } }),
    (error) => expectCribra(error, "invalid_argument", "share"),
  );
});

test("Core rejects Share Bundles without a native builder", { skip: flow }, () => {
  const capabilities = api.getCapabilities();
  assert.equal(capabilities.profile, "core");
  assert.equal(capabilities.shareBundle, false);
  assert.equal("ShareBundleBuilder" in native, false);
  assert.throws(
    () => api.createShareBundle(scanner, [{ id: "x", source: "clean" }], {
      mode: { kind: "redact" },
    }),
    (error) => expectCribra(error, "unsupported_capability", "capability"),
  );
  const disposable = api.createScanner();
  disposable.dispose();
  assert.throws(
    () => api.createShareBundle(disposable, [], { mode: { kind: "redact" } }),
    (error) => expectCribra(error, "disposed", "scanner"),
  );
});
