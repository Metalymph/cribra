/* Installed-package Node ESM consumer suite. Imports only by package name. */

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

import * as api from "cribra-typescript";

const require = createRequire(import.meta.url);
const packageJsonPath = require.resolve("cribra-typescript/package.json");
const packageDir = dirname(packageJsonPath);
const packageManifest = JSON.parse(await readFile(packageJsonPath, "utf8"));
const wasmPath = resolve(packageDir, "dist/wasm/cribra_bg.wasm");

const argv = process.argv.slice(2);
function flag(name, fallback) {
  const index = argv.indexOf(`--${name}`);
  if (index < 0) return fallback;
  const value = argv[index + 1];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`--${name} requires a value`);
  }
  return value;
}
function has(name) {
  return argv.includes(`--${name}`);
}

const profile = flag("profile", "flow");
const asset = flag("asset", "default");
const quick = has("quick");
const expectedPackage = process.env.CRIBRA_EXPECTED_PACKAGE ?? packageManifest.version;
const expectedAdapter = process.env.CRIBRA_EXPECTED_ADAPTER;

let passed = 0;
const failures = [];

async function test(name, operation) {
  try {
    await operation();
    passed += 1;
    process.stdout.write(`PASS ${name}\n`);
  } catch (error) {
    failures.push({ name, error });
    process.stderr.write(
      `FAIL ${name}: ${error instanceof Error ? error.stack : String(error)}\n`,
    );
  }
}

function equal(actual, expected, message) {
  assert.equal(actual, expected, message);
}

function deepEqual(actual, expected, message) {
  assert.deepEqual(actual, expected, message);
}

function ok(value, message) {
  assert.ok(value, message);
}

function expectCribra(operation, code, domain, forbidden = []) {
  try {
    operation();
  } catch (error) {
    equal(error.name, "CribraError", "error name");
    equal(error.code, code, "error code");
    if (domain !== undefined) equal(error.domain, domain, "error domain");
    equal("free" in error, false, "error has no native handle");
    const text = `${error.message}\n${error.stack ?? ""}\n${JSON.stringify(error)}`;
    for (const value of forbidden) {
      equal(text.includes(value), false, `error must not expose ${value}`);
    }
    return error;
  }
  throw new Error(`expected CribraError(${code})`);
}

function plainData(value, path = "root") {
  if (value === null) return;
  const type = typeof value;
  if (
    type === "string" ||
    type === "number" ||
    type === "boolean" ||
    type === "bigint"
  ) {
    return;
  }
  if (type === "undefined") throw new Error(`undefined at ${path}`);
  if (Array.isArray(value)) {
    if (Object.getPrototypeOf(value) !== Array.prototype) {
      throw new Error(`non-plain array at ${path}`);
    }
    value.forEach((entry, index) => plainData(entry, `${path}[${index}]`));
    return;
  }
  if (type !== "object") throw new Error(`non-plain value ${type} at ${path}`);
  if (Object.getPrototypeOf(value) !== Object.prototype) {
    throw new Error(`non-plain object at ${path}`);
  }
  for (const [key, entry] of Object.entries(value)) {
    if (key === "free" || typeof entry === "function") {
      throw new Error(`native handle or function at ${path}.${key}`);
    }
    plainData(entry, `${path}.${key}`);
  }
}

function assertNoRust() {
  if (process.env.CRIBRA_EXPECT_NO_RUST !== "1") return;
  for (const tool of ["cargo", "wasm-bindgen"]) {
    const result = spawnSync(tool, ["--version"], {
      stdio: "ignore",
      env: process.env,
    });
    if (result.error === undefined && result.status === 0) {
      throw new Error(`consumer runtime unexpectedly has ${tool} on PATH`);
    }
  }
}

async function startWasmServer(bytes) {
  const server = createServer((request, response) => {
    if (!request.url?.endsWith("/cribra_bg.wasm")) {
      response.writeHead(404).end("not found");
      return;
    }
    response.writeHead(200, {
      "Content-Type": "application/wasm",
      "Cache-Control": "no-store",
    });
    response.end(bytes);
  });
  await new Promise((resolvePromise, rejectPromise) => {
    server.once("error", rejectPromise);
    server.listen(0, "127.0.0.1", resolvePromise);
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("HTTP server did not expose a port");
  }
  return {
    url: `http://127.0.0.1:${address.port}/cribra_bg.wasm`,
    close: () => new Promise((resolvePromise) => server.close(resolvePromise)),
  };
}

const customRules = [
  {
    id: "test.literal",
    kind: "literal",
    value: "custom-secret-123",
    severity: "high",
    remediation: "rotate_credential",
  },
  {
    id: "test.prefix",
    kind: "prefix",
    value: "PRE_",
    severity: "medium",
  },
  {
    id: "test.suffix",
    kind: "suffix",
    value: "_END",
    severity: "low",
    remediation: null,
  },
  {
    id: "test.pattern",
    kind: "pattern",
    pattern: "token-[0-9]{4}",
    severity: "critical",
    remediation: "remove_sensitive_value",
  },
];

const customSource = "custom-secret-123\nPRE_beta\nalpha_END\ntoken-1234";
const token = "ghp_AbCdEf0123456789_AbCdEf0123456789";
const privacySentinel = "SCANNED_SECRET_DO_NOT_EXPOSE";
const encoder = new TextEncoder();

function customScannerOptions() {
  return { builtins: "none", rules: customRules };
}

async function initialize() {
  if (asset === "default") {
    await assert.rejects(
      api.init({ wasm: new Uint8Array([0x00, 0x01, 0x02, 0x03]) }),
      (error) => {
        equal(error.name, "CribraError", "invalid init error name");
        ok(
          error.code === "wasm_load_failed" ||
            error.code === "wasm_instantiate_failed",
          "invalid init uses a structured WASM error",
        );
        equal(
          `${error.message}${error.stack ?? ""}`.includes(privacySentinel),
          false,
          "invalid init does not expose source",
        );
        return true;
      },
    );
    await Promise.all([api.init(), api.init(), api.init()]);
  } else if (asset === "bytes") {
    await api.init({ wasm: new Uint8Array(await readFile(wasmPath)) });
  } else if (asset === "arraybuffer") {
    const bytes = await readFile(wasmPath);
    await api.init({
      wasm: bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength),
    });
  } else if (asset === "file-url") {
    await api.init({ wasm: pathToFileURL(wasmPath) });
  } else if (asset === "path") {
    await api.init({ wasm: wasmPath });
  } else if (asset === "http-url") {
    const bytes = await readFile(wasmPath);
    const server = await startWasmServer(bytes);
    try {
      await api.init({ wasm: server.url });
    } finally {
      await server.close();
    }
  } else {
    throw new Error(`unknown --asset ${asset}`);
  }
}

async function main() {
  assertNoRust();

  equal(api.isInitialized(), false, "import performs no initialization");
  expectCribra(() => api.createScanner(), "not_initialized", "init");
  expectCribra(() => api.getCapabilities(), "not_initialized", "init");
  expectCribra(() => api.getVersion(), "not_initialized", "init");

  await initialize();
  equal(api.isInitialized(), true, "initialized after init");
  await api.init();
  equal(api.isInitialized(), true, "repeated init is idempotent");

  const capabilities = api.getCapabilities();
  equal(capabilities.backend, "wasm", "backend");
  equal(capabilities.profile, profile, "profile");
  equal(capabilities.parallel, false, "parallel");
  equal(capabilities.singleSourceScan, true, "single scan");
  equal(capabilities.batchScan, true, "batch scan");
  equal(capabilities.streaming, true, "streaming");
  equal(capabilities.incremental, true, "incremental");
  equal(capabilities.ruleMetadata, true, "rule metadata");
  equal(capabilities.explanations, true, "explanations");
  equal(capabilities.utf16OffsetConversion, true, "offsets");
  equal(
    capabilities.redaction,
    profile === "flow",
    "redaction profile gate",
  );
  equal(
    capabilities.templating,
    profile === "flow",
    "templating profile gate",
  );
  equal(
    capabilities.pseudonymization,
    profile === "flow",
    "pseudonymization profile gate",
  );
  equal(
    capabilities.synthesis,
    profile === "flow",
    "synthesis profile gate",
  );
  equal(capabilities.shareBundle, profile === "flow", "share bundle gate");

  const version = api.getVersion();
  equal(version.package, expectedPackage, "package version");
  ok(typeof version.wasmAdapter === "string" && version.wasmAdapter.length > 0, "adapter version");
  equal("core" in version, false, "no fabricated core version");
  if (expectedAdapter !== undefined) {
    equal(version.wasmAdapter, expectedAdapter, "adapter version matches source");
  }

  if (quick || asset !== "default") {
    const scanner = api.createScanner();
    const result = scanner.scan(`GITHUB_TOKEN=${token}`);
    ok(
      result.findings.some((finding) => finding.ruleId === "github.classic-pat"),
      "built-in finding after asset initialization",
    );
    scanner.dispose();
    passed += 1;
    return;
  }

  await test("built-in scan", () => {
    const scanner = api.createScanner();
    const result = scanner.scan(`GITHUB_TOKEN=${token}`);
    ok(
      result.findings.some((finding) => finding.ruleId === "github.classic-pat"),
      "built-in finding",
    );
    scanner.dispose();
  });

  const customScanner = api.createScanner(customScannerOptions());
  const customResult = customScanner.scan(customSource, {
    includeExplanations: true,
  });

  await test("custom rules and explanations", () => {
    equal(customScanner.rulesCount, customRules.length, "custom rule count");
    equal(customResult.findingCount, 4, "custom finding count");
    deepEqual(
      customResult.findings.map((finding) => finding.ruleId),
      ["test.literal", "test.prefix", "test.suffix", "test.pattern"],
      "custom rule order",
    );
    equal(customResult.findings[0].span.start, 0, "literal start");
    equal(customResult.findings[0].span.end, 17, "literal end");
    equal(customResult.findings[0].line, 1, "literal line");
    equal(customResult.findings[0].column, 1, "literal column");
    equal(customResult.findings[0].severity, "high", "literal severity");
    equal(
      customResult.findings[0].remediation,
      "rotate_credential",
      "literal remediation",
    );
    equal(customResult.findings[0].explanation.kind, "classified", "explanation");
    equal(
      customResult.findings[0].explanation.detectionMode,
      "matcher_only",
      "detection mode",
    );
  });

  await test("batch scan", () => {
    const batch = customScanner.scanBatch([
      { id: "", source: customSource },
      { id: "", source: "clean" },
      { id: "unicode-🦀", source: "token-9999" },
    ]);
    equal(batch.entries.length, 3, "batch length");
    deepEqual(
      batch.entries.map((entry) => entry.id),
      ["", "", "unicode-🦀"],
      "batch IDs",
    );
    equal(batch.entries[0].result.findingCount, 4, "batch first result");
    equal(batch.summary.scannedSources, 3, "summary sources");
    equal(batch.summary.totalFindings, 5, "summary findings");
  });

  await test("streaming and split UTF-8", () => {
    const bytes = encoder.encode(customSource);
    const split = bytes.length - 3;
    const stream = customScanner.stream();
    stream.push(bytes.subarray(0, split));
    stream.push(new Uint8Array());
    stream.push(bytes.subarray(split));
    const streamed = stream.finish();
    deepEqual(streamed, customScanner.scan(customSource), "stream matches scan");
    equal(streamed.findingCount, 4, "stream finding count");
    equal(stream.disposed, true, "finished stream disposed");
    expectCribra(() => stream.push(new Uint8Array()), "stream_not_active", "stream");
    stream.dispose();

    const unicodeScanner = api.createScanner({
      builtins: "none",
      rules: [
        { id: "unicode.stream", kind: "literal", value: "秘密🔑", severity: "high" },
      ],
    });
    const unicodeSource = "前🙂\r\n秘密🔑";
    const unicodeBytes = encoder.encode(unicodeSource);
    const unicodeStream = unicodeScanner.stream();
    const emojiSplit = encoder.encode("前🙂").length - 2;
    const crlfSplit = encoder.encode("前🙂\r").length;
    unicodeStream.push(unicodeBytes.subarray(0, emojiSplit));
    unicodeStream.push(unicodeBytes.subarray(emojiSplit, crlfSplit));
    unicodeStream.push(unicodeBytes.subarray(crlfSplit));
    deepEqual(
      unicodeStream.finish(),
      unicodeScanner.scan(unicodeSource),
      "unicode stream matches scan",
    );
    unicodeStream.dispose();
    unicodeScanner.dispose();
  });

  await test("incremental scanning", () => {
    const incremental = customScanner.incremental(customSource);
    const initial = incremental.snapshot();
    equal(initial.revision, 0n, "initial revision");
    equal(initial.result.findingCount, 4, "initial result");
    const revision = incremental.applyEdit(0n, 0, 0, "ADDED ");
    equal(revision, 1n, "insert revision");
    deepEqual(
      incremental.snapshot().result,
      customScanner.scan(`ADDED ${customSource}`),
      "incremental insert matches scan",
    );
    const noop = incremental.applyEdit(1n, 0, 0, "");
    equal(noop, 2n, "no-op revision");
    const after = incremental.snapshot();
    equal(after.revision, 2n, "snapshot revision");
    equal(initial.revision, 0n, "old snapshot revision");
    incremental.dispose();
    incremental.dispose();
    expectCribra(() => incremental.snapshot(), "disposed", "incremental");
    expectCribra(
      () => incremental.applyEdit(2n, 0, 0, ""),
      "disposed",
      "incremental",
    );
  });

  await test("terminal incremental failure", () => {
    const scanner = api.createScanner({
      builtins: "none",
      rules: [{ id: "terminal", kind: "literal", value: "SECRET", severity: "high" }],
    });
    const session = scanner.incremental("", { limits: { maxCandidates: 0 } });
    const before = session.snapshot();
    expectCribra(
      () => session.applyEdit(0n, 0, 0, "SECRET"),
      "incremental_candidate_limit",
      "incremental",
    );
    equal(session.isActive, false, "terminal session inactive");
    equal(session.disposed, false, "terminal session not disposed");
    equal(session.snapshot().revision, before.revision, "terminal snapshot revision");
    deepEqual(session.snapshot().result, before.result, "terminal snapshot preserved");
    session.dispose();
    scanner.dispose();
  });

  await test("offset conversion", () => {
    const source = "aé🦀z";
    deepEqual(
      api.byteSpanToTextRange(source, { start: 0, end: 0 }),
      { start: 0, end: 0 },
      "empty span",
    );
    deepEqual(
      api.byteSpanToTextRange(source, {
        start: 0,
        end: encoder.encode(source).length,
      }),
      { start: 0, end: source.length },
      "whole span",
    );
    deepEqual(
      api.byteSpanToTextRange(source, { start: 1, end: 3 }),
      { start: 1, end: 2 },
      "accent span",
    );
    deepEqual(
      api.byteSpanToTextRange(source, { start: 3, end: 7 }),
      { start: 2, end: 4 },
      "supplementary span",
    );
    expectCribra(
      () => api.byteSpanToTextRange(source, { start: 2, end: 3 }),
      "span_not_utf8_boundary",
      "offset",
    );
  });

  let retainedTransform;
  let retainedBundle;

  if (profile === "flow") {
    await test("Flow transformations", () => {
      const source = "TOKEN=custom-secret-123";
      const redacted = customScanner.transform(source, { kind: "redact" });
      equal(redacted.output, "TOKEN=[REDACTED]", "redaction");
      equal(redacted.scan.findingCount, 1, "transform includes scan");
      equal(redacted.output.includes("custom-secret-123"), false, "secret removed");

      const templated = customScanner.transform(source, {
        kind: "template",
        namespace: "SAFE",
        numbered: true,
      });
      ok(templated.output.startsWith("TOKEN=<SAFE:"), "template namespace");

      const key = new Uint8Array(32).fill(7);
      const pseudonymized = customScanner.transform(source, {
        kind: "pseudonymize",
        key,
      });
      ok(pseudonymized.output.startsWith("TOKEN=cribra_pseudo_"), "pseudonym prefix");
      equal(
        pseudonymized.output,
        customScanner.transform(source, {
          kind: "pseudonymize",
          key,
        }).output,
        "pseudonym deterministic",
      );

      const synthesized = customScanner.transform(source, {
        kind: "synthesize",
        key,
      });
      equal(
        synthesized.output,
        customScanner.transform(source, {
          kind: "synthesize",
          key,
        }).output,
        "synthesis deterministic",
      );
      equal(synthesized.output.includes("custom-secret-123"), false, "synthesis removes secret");

      const unicode = customScanner.transform("前🦀 custom-secret-123", {
        kind: "redact",
      });
      equal(unicode.output, "前🦀 [REDACTED]", "unicode redaction");
      retainedTransform = unicode;

      expectCribra(
        () =>
          customScanner.transform(source, {
            kind: "pseudonymize",
            key: new Uint8Array(31),
          }),
        "invalid_key_length",
        "transform",
      );
    });

    await test("Share Bundle", () => {
      const bundle = api.createShareBundle(
        customScanner,
        [
          { id: "browser.txt", source: "TOKEN=custom-secret-123" },
          { id: "unicode-🦀", source: "前🦀 custom-secret-123" },
        ],
        {
          mode: { kind: "redact" },
          generatedAt: new Date(1_700_000_000_123),
        },
      );
      equal(bundle.entries.length, 2, "bundle length");
      deepEqual(
        bundle.entries.map((entry) => entry.id),
        ["browser.txt", "unicode-🦀"],
        "bundle IDs",
      );
      equal(bundle.entries[0].content, "TOKEN=[REDACTED]", "bundle redaction");
      equal(bundle.entries[1].content, "前🦀 [REDACTED]", "bundle unicode");
      equal(bundle.manifest.mode, "redact", "bundle mode");
      equal(bundle.manifest.generatedAtSeconds, 1_700_000_000, "bundle seconds");
      equal(bundle.manifest.generatedAtNanoseconds, 123_000_000, "bundle nanos");
      equal(bundle.summary.scannedSources, 2, "bundle summary sources");
      equal(bundle.summary.totalFindings, 2, "bundle summary findings");
      retainedBundle = bundle;
    });
  } else {
    await test("Core rejects Flow operations", () => {
      expectCribra(
        () => customScanner.transform(customSource, { kind: "redact" }),
        "unsupported_capability",
        "capability",
      );
      expectCribra(
        () =>
          api.createShareBundle(customScanner, [], {
            mode: { kind: "redact" },
          }),
        "unsupported_capability",
        "capability",
      );
    });
  }

  await test("privacy and structured failures", () => {
    if (profile === "flow") {
      const scanner = api.createScanner({
        builtins: "none",
        rules: [
          { id: "privacy.long", kind: "literal", value: privacySentinel, severity: "high" },
          { id: "privacy.short", kind: "literal", value: "SCANNED", severity: "high" },
        ],
      });
      try {
        expectCribra(
          () =>
            scanner.transform(privacySentinel, {
              kind: "template",
              namespace: "PRIVATE",
            }),
          "transform_overlapping_spans",
          "transform",
          [privacySentinel, "PRIVATE"],
        );
      } finally {
        scanner.dispose();
      }

      const timestampScanner = api.createScanner({
        builtins: "none",
        rules: [
          { id: "privacy.timestamp", kind: "literal", value: privacySentinel, severity: "high" },
        ],
      });
      try {
        expectCribra(
          () =>
            api.createShareBundle(
              timestampScanner,
              [{ id: "privacy", source: privacySentinel }],
              {
                mode: { kind: "redact" },
                generatedAt: new Date(Number.NaN),
              },
            ),
          "share_timestamp_invalid",
          "share",
          [privacySentinel],
        );
      } finally {
        timestampScanner.dispose();
      }
    }
    expectCribra(
      () => api.byteSpanToTextRange("bad\uD800", { start: 0, end: 0 }),
      "invalid_unicode_string",
      "offset",
      ["bad"],
    );
  });

  await test("lifecycle and DTO independence", () => {
    customScanner.dispose();
    customScanner.dispose();
    equal(customScanner.disposed, true, "disposed flag");
    equal(customResult.findingCount, 4, "result valid after dispose");
    equal(customResult.findings[0].span.start, 0, "span valid after dispose");
    equal(customScanner.rules.length, 4, "metadata valid after dispose");
    expectCribra(() => customScanner.scan("token-1234"), "disposed", "scanner");
    expectCribra(() => customScanner.scanBatch([]), "disposed", "scanner");
    if (retainedTransform !== undefined) {
      equal(retainedTransform.output, "前🦀 [REDACTED]", "transform valid after dispose");
      equal(retainedTransform.scan.findingCount, 1, "transform scan valid after dispose");
    }
    if (retainedBundle !== undefined) {
      equal(retainedBundle.entries[1].content, "前🦀 [REDACTED]", "bundle valid after dispose");
      equal(retainedBundle.manifest.mode, "redact", "bundle manifest valid after dispose");
      equal(retainedBundle.summary.totalFindings, 2, "bundle summary valid after dispose");
    }
  });

  await test("plain data and no native handles", () => {
    plainData(customResult, "result");
    plainData(retainedTransform ?? {}, "transform");
    plainData(retainedBundle ?? {}, "bundle");
  });

  await test("scanner metadata", () => {
    const scanner = api.createScanner();
    ok(scanner.rulesCount > 0, "default rules");
    equal(scanner.rules.length, scanner.rulesCount, "rules length");
    ok(
      scanner.rules.some((rule) => rule.ruleId === "github.classic-pat"),
      "default rule present",
    );
    scanner.dispose();
  });
}

await main();

if (failures.length > 0) {
  process.stderr.write(
    `\n${failures.length} failure(s), ${passed} passed\n`,
  );
  process.exitCode = 1;
} else {
  process.stdout.write(`\nall ${passed} checks passed (${profile}, asset=${asset})\n`);
}
