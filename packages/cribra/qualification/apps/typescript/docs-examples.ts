/*
 * Installed-package examples used by the public TypeScript documentation.
 *
 * Each marker maps a published fenced example title in
 * website/src/content/docs/typescript*.md to executable coverage in this
 * fixture. The website content validator requires both sets to match.
 *
 * docs-example: initialize.ts
 * docs-example: scan.ts
 * docs-example: batch.ts
 * docs-example: offsets.ts
 * docs-example: errors-and-disposal.ts
 * docs-example: stream.ts
 * docs-example: incremental.ts
 * docs-example: transformations.ts
 * docs-example: share-bundle.ts
 */

import {
  CribraError,
  byteSpanToTextRange,
  createScanner,
  createShareBundle,
  getCapabilities,
  getVersion,
  init,
  isInitialized,
} from "__CRIBRA_PACKAGE_SPECIFIER__";
import type {
  RuleSpec,
  ScanSource,
  TransformRequest,
} from "__CRIBRA_PACKAGE_SPECIFIER__";

const encoder = new TextEncoder();
const key = new Uint8Array(32).fill(0x2a);
const rules: readonly RuleSpec[] = [
  {
    id: "demo.api-key",
    kind: "prefix",
    value: "demo_live_",
    severity: "critical",
    remediation: "rotate_credential",
  },
  {
    id: "demo.secret",
    kind: "literal",
    value: "demo-secret-123",
    severity: "high",
  },
];
const scannerOptions = { builtins: "none" as const, rules };
const source = "ACME_API_KEY=demo_live_7f3a91\nnote=demo-secret-123\n";

function assert(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

function equal<T>(actual: T, expected: T, message: string): void {
  if (actual !== expected) {
    throw new Error(
      `${message}: expected ${String(expected)}, received ${String(actual)}`,
    );
  }
}

function expectCode(operation: () => unknown, code: string, domain: string): void {
  try {
    operation();
  } catch (error) {
    assert(error instanceof CribraError, `expected CribraError(${code})`);
    equal(error.code, code, `${code} code`);
    equal(error.domain, domain, `${code} domain`);
    return;
  }
  throw new Error(`expected CribraError(${code})`);
}

async function exampleInitialization(): Promise<void> {
  equal(isInitialized(), false, "import performs no initialization");
  expectCode(() => createScanner({ builtins: "none" }), "not_initialized", "init");
  expectCode(() => getCapabilities(), "not_initialized", "init");
  expectCode(() => getVersion(), "not_initialized", "init");

  await init();
  equal(isInitialized(), true, "initialized after init");
  await init();
  equal(isInitialized(), true, "repeated init is idempotent");

  const capabilities = getCapabilities();
  equal(capabilities.backend, "wasm", "backend");
  equal(capabilities.profile, "flow", "profile");
  assert(capabilities.singleSourceScan, "single-source scanning");
  assert(capabilities.batchScan, "batch scanning");
  assert(capabilities.streaming, "streaming");
  assert(capabilities.incremental, "incremental");
  assert(capabilities.redaction, "redaction");
  assert(capabilities.templating, "templating");
  assert(capabilities.pseudonymization, "pseudonymization");
  assert(capabilities.synthesis, "synthesis");
  assert(capabilities.shareBundle, "share bundles");
  assert(capabilities.utf16OffsetConversion, "offset conversion");

  const version = getVersion();
  assert(version.package.length > 0, "package version");
}

function exampleScanning(): void {
  const scanner = createScanner(scannerOptions);
  try {
    const result = scanner.scan(source, { includeExplanations: true });
    equal(result.findingCount, 2, "finding count");
    equal(result.sourceBytes, encoder.encode(source).length, "source byte length");
    assert(
      result.findings.every((finding) => finding.explanation !== undefined),
      "requested explanations",
    );

    const finding = result.findings.find((entry) => entry.ruleId === "demo.secret");
    assert(finding, "literal finding");
    equal(
      source.slice(finding.span.start, finding.span.end),
      "demo-secret-123",
      "ASCII byte span",
    );
    assert(
      JSON.stringify(result).includes("demo-secret-123") === false,
      "reports retain no matched values",
    );

    const range = byteSpanToTextRange(source, finding.span);
    equal(
      source.slice(range.start, range.end),
      "demo-secret-123",
      "UTF-16 text range",
    );
  } finally {
    scanner.dispose();
  }
}

function exampleUtf8Offsets(): void {
  const unicodeSource = "🦀 demo-secret-123";
  const startIndex = unicodeSource.indexOf("demo-secret-123");
  const start = encoder.encode(unicodeSource.slice(0, startIndex)).length;
  const end = start + encoder.encode("demo-secret-123").length;

  const range = byteSpanToTextRange(unicodeSource, { start, end });
  equal(unicodeSource.slice(range.start, range.end), "demo-secret-123", "scalar alignment");
}

function exampleBatch(): void {
  const scanner = createScanner(scannerOptions);
  try {
    const sources: readonly ScanSource[] = [
      { id: "", source: "clean" },
      { id: "same", source: "demo-secret-123" },
      { id: "same", source: "no findings" },
      { id: "unicode-🦀", source: "ACME_API_KEY=demo_live_7f3a91" },
    ];
    const batch = scanner.scanBatch(sources);
    equal(batch.entries.length, sources.length, "one entry per source");
    equal(
      batch.entries.map((entry) => entry.id).join("|"),
      sources.map((entry) => entry.id).join("|"),
      "ID order and duplicates",
    );
    equal(batch.summary.scannedSources, 4, "summary source count");
    equal(batch.summary.totalFindings, 2, "summary finding count");
    equal(batch.summary.isClean, false, "summary cleanliness");
  } finally {
    scanner.dispose();
  }
}

function exampleStreaming(): void {
  const scanner = createScanner({
    builtins: "none",
    rules: [
      {
        id: "demo.stream",
        kind: "literal",
        value: "demo-secret-123",
        severity: "high",
      },
    ],
  });
  try {
    const streamSource = "prefix 🦀 demo-secret-123 suffix";
    const bytes = encoder.encode(streamSource);
    const session = scanner.stream();
    try {
      // Split inside the four-byte emoji scalar.
      const split = encoder.encode("prefix ").length + 1;
      session.push(bytes.subarray(0, split));
      session.push(bytes.subarray(split));
      const result = session.finish();
      equal(result.findingCount, 1, "stream finding count");
      assert(session.disposed, "finished stream is inactive");
      expectCode(
        () => session.push(new Uint8Array()),
        "stream_not_active",
        "stream",
      );
    } finally {
      session.dispose();
    }
  } finally {
    scanner.dispose();
  }
}

function exampleIncremental(): void {
  const scanner = createScanner({
    builtins: "none",
    rules: [
      {
        id: "demo.incremental",
        kind: "literal",
        value: "demo-secret-123",
        severity: "high",
      },
    ],
  });
  try {
    const session = scanner.incremental("🦀 demo-secret-123\n");
    try {
      let revision = 0n;
      equal(session.snapshot().revision, revision, "initial revision");

      const start = encoder.encode("🦀 ").length;
      const end = start + encoder.encode("demo-secret-123").length;
      revision = session.applyEdit(
        revision,
        start,
        end,
        "demo-secret-456",
      );
      equal(revision, 1n, "next bigint revision");
      equal(session.snapshot().revision, 1n, "snapshot revision");
      equal(session.snapshot().result.findingCount, 0, "edited report");
      assert(session.isActive, "session remains active");
    } finally {
      session.dispose();
    }
    assert(session.disposed, "disposed incremental session");
  } finally {
    scanner.dispose();
  }
}

function exampleTransformations(): void {
  const scanner = createScanner(scannerOptions);
  try {
    const transformSource = "🦀 demo-secret-123\n";
    const requests: readonly TransformRequest[] = [
      { kind: "redact" },
      { kind: "template", namespace: "DEMO", numbered: true },
      { kind: "pseudonymize", key, prefix: "demo_pseudo_", digestBytes: 16 },
      { kind: "synthesize", key, marker: "demo_synthetic" },
    ];

    for (const request of requests) {
      const result = scanner.transform(transformSource, request);
      equal(result.scan.findingCount, 1, `${request.kind} scan`);
      assert(
        result.output.includes("demo-secret-123") === false,
        `${request.kind} removed the finding`,
      );
      assert(result.output.includes("🦀"), `${request.kind} preserved context`);
    }

    const first = scanner.transform(transformSource, requests[2]!);
    const second = scanner.transform(transformSource, requests[2]!);
    equal(first.output, second.output, "deterministic keyed pseudonym");
  } finally {
    scanner.dispose();
  }
}

function exampleShareBundle(): void {
  const scanner = createScanner(scannerOptions);
  try {
    const generatedAt = new Date("2026-01-01T00:00:00Z");
    const sources: readonly ScanSource[] = [
      { id: "config.env", source: "ACME_API_KEY=demo_live_7f3a91\n" },
      { id: "notes.txt", source: "clean\n" },
    ];
    const bundle = createShareBundle(scanner, sources, {
      mode: { kind: "redact" },
      generatedAt,
    });

    equal(bundle.entries.length, 2, "bundle entry count");
    equal(bundle.entries[0]?.id, "config.env", "bundle entry ID");
    assert(
      bundle.entries[0]?.content.includes("demo_live_7f3a91") === false,
      "bundle entry redacted",
    );
    equal(bundle.manifest.mode, "redact", "manifest mode");
    equal(
      bundle.manifest.generatedAtSeconds,
      Math.floor(generatedAt.getTime() / 1000),
      "manifest seconds",
    );
    equal(
      bundle.manifest.generatedAtNanoseconds,
      (generatedAt.getTime() % 1000) * 1_000_000,
      "manifest nanoseconds",
    );
    equal(bundle.summary.scannedSources, 2, "bundle summary");
  } finally {
    scanner.dispose();
  }
}

function exampleErrorsAndDisposal(): void {
  const scanner = createScanner(scannerOptions);
  scanner.dispose();
  assert(scanner.disposed, "scanner disposed");
  expectCode(() => scanner.scan(source), "disposed", "scanner");

  const sessionScanner = createScanner(scannerOptions);
  const session = sessionScanner.incremental("clean");
  sessionScanner.dispose();
  assert(session.isActive, "session outlives the scanner");
  equal(session.snapshot().result.findingCount, 0, "session snapshot after scanner disposal");
  session.dispose();
  assert(session.disposed, "session disposed");
}

async function main(): Promise<void> {
  await exampleInitialization();
  exampleScanning();
  exampleUtf8Offsets();
  exampleBatch();
  exampleStreaming();
  exampleIncremental();
  exampleTransformations();
  exampleShareBundle();
  exampleErrorsAndDisposal();
  console.log("TypeScript documentation examples passed");
}

await main();
