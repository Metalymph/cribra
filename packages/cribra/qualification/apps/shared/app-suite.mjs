/* Real Cribra operations executed inside a production browser bundle. */

export async function runAppSuite(api, { expectedVersion } = {}) {
  if (!Object.is(api.isInitialized(), false)) {
    throw new Error("import initialized WASM before init()");
  }
  await Promise.all([api.init(), api.init()]);
  if (!Object.is(api.isInitialized(), true)) throw new Error("init failed");

  const capabilities = api.getCapabilities();
  if (capabilities.profile !== "flow") throw new Error("expected Flow profile");
  if (capabilities.parallel !== false) throw new Error("parallel must stay false");
  const version = api.getVersion();
  if (expectedVersion !== undefined && version.package !== expectedVersion) {
    throw new Error(`package version mismatch: ${version.package}`);
  }

  const rules = [
    {
      id: "bundle.literal",
      kind: "literal",
      value: "custom-secret-123",
      severity: "high",
      remediation: "rotate_credential",
    },
    {
      id: "bundle.pattern",
      kind: "pattern",
      pattern: "token-[0-9]{4}",
      severity: "critical",
    },
  ];
  const source = "custom-secret-123\ntoken-1234";
  const scanner = api.createScanner({ builtins: "none", rules });
  const result = scanner.scan(source, { includeExplanations: true });
  if (result.findingCount !== 2) throw new Error(`scan findings: ${result.findingCount}`);
  if (result.findings[0].ruleId !== "bundle.literal") {
    throw new Error("scan rule order");
  }

  const batch = scanner.scanBatch([
    { id: "a", source },
    { id: "b", source: "clean" },
  ]);
  if (batch.entries.length !== 2 || batch.summary.totalFindings !== 2) {
    throw new Error("batch mismatch");
  }

  const encoder = new TextEncoder();
  const bytes = encoder.encode(source);
  const stream = scanner.stream();
  stream.push(bytes.subarray(0, bytes.length - 3));
  stream.push(bytes.subarray(bytes.length - 3));
  const streamed = stream.finish();
  if (streamed.findingCount !== 2) throw new Error("stream mismatch");
  stream.dispose();

  const incremental = scanner.incremental(source);
  const revision = incremental.applyEdit(0n, 0, 0, "ADDED ");
  if (revision !== 1n) throw new Error("incremental revision");
  if (incremental.snapshot().result.findingCount !== 2) {
    throw new Error("incremental snapshot");
  }
  incremental.dispose();

  const key = new Uint8Array(32).fill(9);
  const redacted = scanner.transform("TOKEN=custom-secret-123", { kind: "redact" });
  if (redacted.output !== "TOKEN=[REDACTED]") {
    throw new Error(`redaction: ${redacted.output}`);
  }
  const templated = scanner.transform("TOKEN=custom-secret-123", {
    kind: "template",
    namespace: "SAFE",
  });
  if (!templated.output.startsWith("TOKEN=<SAFE:")) throw new Error("template");
  const pseudonymized = scanner.transform("TOKEN=custom-secret-123", {
    kind: "pseudonymize",
    key,
  });
  if (!pseudonymized.output.startsWith("TOKEN=cribra_pseudo_")) {
    throw new Error("pseudonymization");
  }
  const synthesized = scanner.transform("TOKEN=custom-secret-123", {
    kind: "synthesize",
    key,
  });
  if (synthesized.output.includes("custom-secret-123")) throw new Error("synthesis");

  const bundle = api.createShareBundle(
    scanner,
    [
      { id: "a.txt", source: "TOKEN=custom-secret-123" },
      { id: "b.txt", source: "token-1234" },
    ],
    { mode: { kind: "redact" }, generatedAt: new Date(1_700_000_000_123) },
  );
  if (bundle.entries.length !== 2) throw new Error("bundle length");
  if (bundle.entries[0].content !== "TOKEN=[REDACTED]") {
    throw new Error(`bundle redaction: ${bundle.entries[0].content}`);
  }
  if (bundle.manifest.generatedAtSeconds !== 1_700_000_000) {
    throw new Error("bundle timestamp");
  }

  const range = api.byteSpanToTextRange("aé🦀z", { start: 3, end: 7 });
  if (range.start !== 2 || range.end !== 4) throw new Error("offset conversion");

  try {
    api.byteSpanToTextRange("aé🦀z", { start: 2, end: 3 });
    throw new Error("expected span error");
  } catch (error) {
    if (error.code !== "span_not_utf8_boundary") throw error;
  }

  scanner.dispose();
  scanner.dispose();
  if (!scanner.disposed) throw new Error("dispose flag");
  try {
    scanner.scan("token-1234");
    throw new Error("expected disposed error");
  } catch (error) {
    if (error.code !== "disposed") throw error;
  }

  return {
    packageVersion: version.package,
    adapterVersion: version.wasmAdapter,
    findings: result.findingCount,
    bundleEntries: bundle.entries.length,
  };
}
