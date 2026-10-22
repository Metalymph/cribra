import assert from "node:assert/strict";
import { qualifyStreams } from "./stream.mjs";
import { qualifyIncremental } from "./incremental.mjs";
import { qualifyBatch } from "./batch.mjs";
import { assertShareAbsent, qualifyShareBundles, shareExports } from "./share_bundle.mjs";
import { qualifyRemediation } from "./remediation.mjs";
import { qualifyConfiguration } from "./configuration.mjs";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [variant, directory] = process.argv.slice(2);
assert(["core", "flow"].includes(variant), "Expected core or flow");
assert(directory, "Expected generated bindings directory");
const transformations = new Set([
  "redact", "redactWith", "template", "templateWith", "pseudonymize", "synthesize",
]);
const contract = JSON.parse(await readFile(new URL("api-contract.json", import.meta.url), "utf8"));
const declarations = (await readFile(resolve(directory, "cribra.d.ts"), "utf8"))
  .replace(/\/\*[\s\S]*?\*\//g, "");
const actual = {};
for (const [, kind, name, base, body] of declarations.matchAll(/export (class|enum|interface) (\w+)(?: extends (\w+))? \{([^}]+)\}/g)) {
  // Keep wasm-bindgen's low-level initialization ABI outside the adapter DTO inventory.
  if (kind === "interface" && name === "InitOutput") continue;
  actual[name] = {
    kind,
    ...(base ? { extends: base } : {}),
    members: body.split(kind === "enum" ? "," : ";")
      .map((member) => member.trim().replace(/\s+/g, " "))
      .filter((member) => member && !/^(private constructor|free\(|\[Symbol.dispose\])/.test(member)),
  };
}
const expected = structuredClone(contract);
if (variant === "core") {
  expected.ScanResult.members = expected.ScanResult.members.filter(
    (member) => !transformations.has(member.split("(")[0]),
  );
  for (const name of shareExports) delete expected[name];
}
assert.deepEqual(actual, expected, `${variant}: generated TypeScript contract changed`);

const mod = await import(pathToFileURL(resolve(directory, "cribra.js")).href);
await mod.default({ module_or_path: await readFile(resolve(directory, "cribra_bg.wasm")) });

// Check the actual JS exports and prototypes as well as the TypeScript text.
for (const [name, { kind, members }] of Object.entries(expected)) {
  if (kind === "interface") {
    assert.equal(name in mod, false, `${name} must remain a type-only interface`);
    continue;
  }
  assert(mod[name], `Missing JS export: ${name}`);
  if (kind === "enum") {
    for (const member of members) {
      const [key, value] = member.split(/\s*=\s*/);
      assert.equal(mod[name][key], Number(value), `${name}.${key}`);
    }
    continue;
  }
  assert.equal(typeof mod[name].prototype.free, "function", `${name}.free`);
  assert.equal(mod[name].prototype.free.length, 0, `${name}.free arity`);
  for (const member of members) {
    if (member.startsWith("readonly ")) {
      const key = member.slice("readonly ".length).split(":")[0];
      assert.equal(typeof Object.getOwnPropertyDescriptor(mod[name].prototype, key)?.get, "function");
    } else {
      const [, key, parameters] = member.match(/^(\w+)\(([^)]*)\)/);
      const fn = key === "constructor" ? mod[name] : mod[name].prototype[key];
      assert.equal(typeof fn, "function", `${name}.${key}`);
      assert.equal(fn.length, parameters ? parameters.split(",").length : 0, `${name}.${key} arity`);
    }
  }
}
for (const method of transformations) {
  assert.equal(method in mod.ScanResult.prototype, variant === "flow", `${variant}: ${method}`);
}
if (variant === "core") assertShareAbsent(mod);

const builder = new mod.ScanEngineBuilder(false);
builder.addLiteral("custom.secret", "SECRET", mod.FindingSeverity.High);
assert.equal(builder.customRuleCount(), 1);
const engine = builder.build();
assert.equal(engine.rulesCount(), 1);
const source = "é SECRET";
const result = engine.scan(source);
assert.equal(result.sourceBytes, new TextEncoder().encode(source).length);
assert.equal(result.findingCount(), 1);
assert.equal(result.candidateCount(), 0);
assert.equal(result.needsReview(), true);
assert.equal(result.hasCritical(), false);
const finding = result.findingAt(0);
assert.equal(finding.ruleId, "custom.secret");
assert.deepEqual([finding.start, finding.end, finding.line, finding.column], [3, 9, 1, 3]);
assert.equal(finding.severity, mod.FindingSeverity.High);
const explanation = engine.explainFinding(result, 0);
assert.equal(explanation.kind, mod.ExplanationKind.Classified);
assert.equal(explanation.detectionMode, mod.DetectionModeKind.MatcherOnly);
assert.throws(() => result.findingAt(1));

const builtin = new mod.ScanEngine();
assert(builtin.rulesCount() > 0);
const detected = builtin.scan("ghp_abcdefghijklmnopqrstuvwxyzABCDEFGHIJ");
assert.equal(detected.findingCount(), 1);
const review = builtin.scan("ABCD-EFGH-IJKL-MNOP");
assert.equal(review.findingCount(), 0);
assert.equal(review.candidateCount(), 1);
assert.equal(review.needsReview(), true);
const candidate = review.candidateAt(0);
assert.equal(candidate.kind, mod.CandidateKind.RecoveryLikeCode);
const evidence = review.candidateExplanationAt(0);
assert.equal(evidence.kind, mod.ExplanationKind.Ambiguous);
assert.equal(evidence.candidateEvidence, mod.CandidateEvidenceKind.Structural);

if (variant === "flow") {
  assert.equal(result.redact(source), "é [REDACTED]");
  assert.equal(result.redactWith(source, "<hidden>"), "é <hidden>");
  const templated = result.template(source);
  assert(templated.startsWith("é ") && !templated.includes("SECRET"));
  assert(result.templateWith(source, "SAFE", true).includes("<SAFE:"));
  const key = new Uint8Array(32).fill(7);
  const pseudonym = result.pseudonymize(source, key, "pseudo_", 16);
  assert(pseudonym.startsWith("é pseudo_") && !pseudonym.includes("SECRET"));
  assert.equal(result.pseudonymize(source, key, "pseudo_", 16), pseudonym);
  const synthetic = result.synthesize(source, key, "synthetic");
  assert(synthetic.startsWith("é ") && !synthetic.includes("SECRET"));
  assert.equal(result.synthesize(source, key, "synthetic"), synthetic);
  assert.throws(() => result.pseudonymize(source, new Uint8Array(31), "pseudo_", 16));
  assert.throws(() => result.synthesize(source, new Uint8Array(31), "synthetic"));
  assert.equal(review.redact("ABCD-EFGH-IJKL-MNOP"), "ABCD-EFGH-IJKL-MNOP");
}

for (const owned of [finding, explanation, detected, candidate, evidence, review, result, engine, builtin]) {
  owned.free();
}
const streamCases = JSON.parse(await readFile(new URL("stream-cases.json", import.meta.url), "utf8"));
const streamOracle = JSON.parse(await readFile(resolve(directory, "../stream-oracle.json"), "utf8"));
qualifyStreams(mod, variant, streamCases, streamOracle);
const incrementalCases = JSON.parse(await readFile(new URL("incremental-cases.json", import.meta.url), "utf8"));
const incrementalOracle = JSON.parse(await readFile(resolve(directory, "../incremental-oracle.json"), "utf8"));
qualifyIncremental(mod, variant, incrementalCases, incrementalOracle);
const batchCases = JSON.parse(await readFile(new URL("batch-cases.json", import.meta.url), "utf8"));
const batchOracle = JSON.parse(await readFile(resolve(directory, "../batch-oracle.json"), "utf8"));
qualifyBatch(mod, variant, batchCases, batchOracle);
if (variant === "flow") {
  const shareCases = JSON.parse(await readFile(new URL("share-cases.json", import.meta.url), "utf8"));
  const shareOracle = JSON.parse(await readFile(resolve(directory, "../share-oracle.json"), "utf8"));
  qualifyShareBundles(mod, shareCases, shareOracle);
}
const remediationCases = JSON.parse(await readFile(new URL("remediation-cases.json", import.meta.url), "utf8"));
const configurationOracle = JSON.parse(await readFile(resolve(directory, "../configuration-oracle.json"), "utf8"));
qualifyRemediation(mod, variant, remediationCases, configurationOracle);
const configurationCases = JSON.parse(await readFile(new URL("configuration-cases.json", import.meta.url), "utf8"));
const metadataOracle = JSON.parse(await readFile(resolve(directory, "../metadata-oracle.json"), "utf8"));
qualifyConfiguration(mod, variant, configurationCases, metadataOracle);
console.log(`${variant}: generated JS/TypeScript contract and WASM runtime smoke passed`);
