/* Browser smoke test. No Node built-ins are imported. */

const params = new URLSearchParams(location.search);
const expectedProfile = params.get("profile");
const expectedVersion = params.get("version");
const result = document.getElementById("result");

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function equal(actual, expected, message) {
  if (!Object.is(actual, expected)) {
    fail(`${message}: expected ${String(expected)}, got ${String(actual)}`);
  }
}

function deepEqual(actual, expected, message) {
  const left = JSON.stringify(actual);
  const right = JSON.stringify(expected);
  if (left !== right) {
    fail(`${message}: expected ${right}, got ${left}`);
  }
}

function expectCode(operation, code) {
  try {
    operation();
  } catch (error) {
    equal(error.name, "CribraError", "error name");
    equal(error.code, code, "error code");
    return error;
  }
  fail(`expected CribraError(${code})`);
}

const encoder = new TextEncoder();
const byteOffset = (source, index) =>
  encoder.encode(source.slice(0, index)).length;

async function main() {
  const api = await import("/dist/index.browser.js");

  equal(api.isInitialized(), false, "not initialized at import");
  expectCode(() => api.createScanner(), "not_initialized");
  expectCode(() => api.getCapabilities(), "not_initialized");
  expectCode(() => api.getVersion(), "not_initialized");

  await Promise.all([api.init(), api.init(), api.init()]);
  equal(api.isInitialized(), true, "initialized after init");
  await api.init();
  equal(api.isInitialized(), true, "repeated init is idempotent");

  const capabilities = api.getCapabilities();
  equal(capabilities.backend, "wasm", "backend");
  equal(capabilities.profile, expectedProfile, "profile");
  equal(capabilities.singleSourceScan, true, "single source scan");
  equal(capabilities.batchScan, true, "batch scan");
  equal(capabilities.streaming, true, "streaming capability");
  equal(capabilities.incremental, true, "incremental capability");
  equal(capabilities.ruleMetadata, true, "rule metadata");
  equal(capabilities.explanations, true, "explanations");
  equal(capabilities.parallel, false, "parallel");
  equal(capabilities.utf16OffsetConversion, true, "offset helper");
  equal(
    capabilities.redaction,
    expectedProfile === "flow",
    "redaction profile gate",
  );
  equal(
    capabilities.templating,
    expectedProfile === "flow",
    "templating profile gate",
  );
  equal(
    capabilities.shareBundle,
    expectedProfile === "flow",
    "share bundle profile gate",
  );

  const version = api.getVersion();
  equal(version.package, expectedVersion, "package version");
  assert(
    typeof version.wasmAdapter === "string" && version.wasmAdapter.length > 0,
    "adapter version",
  );
  equal("core" in version, false, "no fabricated core version");

  // Default scanner: built-in catalog.
  const defaultScanner = api.createScanner();
  assert(defaultScanner.rulesCount > 0, "default catalog size");
  equal(defaultScanner.rules.length, defaultScanner.rulesCount, "rules count");
  assert(
    defaultScanner.rules.some((rule) => rule.ruleId === "github.classic-pat"),
    "default catalog contains github.classic-pat",
  );
  const token = "ghp_AbCdEf0123456789_AbCdEf0123456789";
  const builtinResult = defaultScanner.scan(`GITHUB_TOKEN=${token}`);
  assert(
    builtinResult.findings.some(
      (finding) => finding.ruleId === "github.classic-pat",
    ),
    "built-in finding",
  );
  defaultScanner.dispose();

  const customRules = [
    {
      id: "browser.literal",
      kind: "literal",
      value: "custom-secret-123",
      severity: "high",
      remediation: "rotate_credential",
    },
    { id: "browser.prefix", kind: "prefix", value: "PRE_", severity: "medium" },
    { id: "browser.suffix", kind: "suffix", value: "_END", severity: "low" },
    {
      id: "browser.pattern",
      kind: "pattern",
      pattern: "token-[0-9]{4}",
      severity: "critical",
      remediation: "remove_sensitive_value",
    },
  ];
  const customSource =
    "custom-secret-123\nPRE_beta\nalpha_END\ntoken-1234";

  // Custom-only scanner must not include built-ins.
  const customScanner = api.createScanner({
    builtins: "none",
    rules: customRules,
  });
  equal(customScanner.rulesCount, customRules.length, "custom-only rules");
  assert(
    !customScanner.rules.some((rule) => rule.ruleId === "github.classic-pat"),
    "custom-only has no built-ins",
  );

  const customResult = customScanner.scan(customSource, {
    includeExplanations: true,
  });
  equal(customResult.findingCount, 4, "custom finding count");
  deepEqual(
    customResult.findings.map((finding) => finding.ruleId),
    [
      "browser.literal",
      "browser.prefix",
      "browser.suffix",
      "browser.pattern",
    ],
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
  equal(customResult.findings[0].confidence, "high", "literal confidence");
  assert(
    customResult.findings[0].explanation !== undefined,
    "finding explanation requested",
  );
  equal(
    customResult.findings[0].explanation.kind,
    "classified",
    "finding explanation kind",
  );
  equal(
    customResult.findings[0].explanation.detectionMode,
    "matcher_only",
    "custom detection mode",
  );

  const withoutExplanations = customScanner.scan(customSource);
  equal(
    "explanation" in withoutExplanations.findings[0],
    false,
    "finding explanation omitted by default",
  );

  // Built-ins plus custom rules compile together.
  const combined = api.createScanner({ rules: [customRules[0]] });
  const combinedSource = `GITHUB_TOKEN=${token}\ncustom-secret-123`;
  const combinedResult = combined.scan(combinedSource);
  const combinedIds = new Set(
    combinedResult.findings.map((finding) => finding.ruleId),
  );
  assert(combinedIds.has("github.classic-pat"), "combined built-in finding");
  assert(combinedIds.has("browser.literal"), "combined custom finding");
  combined.dispose();

  // Candidate detection is rule-independent.
  const candidateScanner = api.createScanner({ builtins: "none" });
  const candidateResult = candidateScanner.scan("ABCD-EFGH-IJKL-MNOP");
  equal(candidateResult.findingCount, 0, "candidate source findings");
  equal(candidateResult.candidateCount, 1, "candidate count");
  equal(candidateResult.needsReview, true, "candidate needs review");
  equal(candidateResult.candidates[0].kind, "recovery_like_code", "candidate kind");
  equal(candidateResult.candidates[0].evidence, "structural", "candidate evidence");
  equal(candidateResult.candidates[0].span.start, 0, "candidate start");
  equal(candidateResult.candidates[0].span.end, 19, "candidate end");
  equal(candidateResult.candidates[0].explanation.kind, "ambiguous", "candidate explanation");

  // Batch order, IDs, and summary.
  const batch = customScanner.scanBatch(
    [
      { id: "", source: customSource },
      { id: "", source: "clean" },
      { id: "unicode-🦀", source: "token-9999" },
    ],
    { includeExplanations: false },
  );
  equal(batch.entries.length, 3, "batch length");
  deepEqual(
    batch.entries.map((entry) => entry.id),
    ["", "", "unicode-🦀"],
    "batch IDs preserved",
  );
  equal(batch.entries[0].result.findingCount, 4, "batch first result");
  equal(
    batch.entries[0].sourceBytes,
    encoder.encode(customSource).length,
    "batch source bytes",
  );
  equal(batch.summary.scannedSources, 3, "summary sources");
  equal(batch.summary.totalFindings, 5, "summary findings");
  equal(batch.summary.hasCandidates, false, "summary candidates");
  equal(batch.summary.isClean, false, "summary clean");

  // Offset conversion across ASCII, accents, and supplementary characters.
  const unicode = "aé🦀z";
  deepEqual(
    api.byteSpanToTextRange(unicode, { start: 0, end: 0 }),
    { start: 0, end: 0 },
    "empty span",
  );
  deepEqual(
    api.byteSpanToTextRange(unicode, { start: 0, end: encoder.encode(unicode).length }),
    { start: 0, end: unicode.length },
    "whole span",
  );
  deepEqual(
    api.byteSpanToTextRange(unicode, { start: 1, end: 3 }),
    { start: 1, end: 2 },
    "accent span",
  );
  deepEqual(
    api.byteSpanToTextRange(unicode, { start: 3, end: 7 }),
    { start: 2, end: 4 },
    "supplementary span",
  );
  expectCode(
    () => api.byteSpanToTextRange(unicode, { start: 2, end: 3 }),
    "span_not_utf8_boundary",
  );
  expectCode(
    () => api.byteSpanToTextRange("\ud800", { start: 0, end: 0 }),
    "invalid_unicode_string",
  );

  // Results stay valid after disposal; scanning afterwards fails.
  customScanner.dispose();
  customScanner.dispose();
  equal(customScanner.disposed, true, "disposed flag");
  equal(customResult.findingCount, 4, "result valid after dispose");
  equal(customResult.findings[0].span.start, 0, "span valid after dispose");
  expectCode(() => customScanner.scan("token-1234"), "disposed");
  expectCode(() => customScanner.scanBatch([]), "disposed");
  equal(customScanner.rules.length, 4, "metadata valid after dispose");
  candidateScanner.dispose();

  result.textContent = "PASS";
}

main().catch((error) => {
  const message = error && error.message ? error.message : String(error);
  const code = error && error.code ? ` [${error.code}]` : "";
  result.textContent = `FAIL: ${message}${code}`;
});
