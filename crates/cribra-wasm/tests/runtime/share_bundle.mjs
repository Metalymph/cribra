// Flow-only generated Share Bundle bindings against an independent Core oracle.
import assert from "node:assert/strict";
import { makeEngine } from "./stream.mjs";
import { projectSummary } from "./batch.mjs";

export const shareExports = [
  "ShareBundleBuilder",
  "ShareBundle",
  "ShareBundleEntry",
  "ShareManifestView",
  "ShareModeKind",
];

function configure(mod, builder, spec) {
  const key = () => new Uint8Array(32).fill(spec.keyByte ?? 7);
  switch (spec.mode) {
    case null:
      break;
    case "redact":
      builder.modeRedact();
      break;
    case "template":
      builder.modeTemplate();
      break;
    case "pseudonymize":
      builder.modePseudonymize(key(), spec.prefix, spec.digestBytes);
      break;
    case "synthesize":
      builder.modeSynthesize(key(), spec.marker);
      break;
    default:
      assert.fail(`unknown share mode: ${spec.mode}`);
  }
}

function timestampMillis(manifest) {
  assert.equal(typeof manifest.generatedAtSeconds, "bigint");
  assert.equal(typeof manifest.generatedAtNanoseconds, "number");
  assert(manifest.generatedAtNanoseconds >= 0);
  assert(manifest.generatedAtNanoseconds < 1_000_000_000);
  return Number(manifest.generatedAtSeconds) * 1000 + manifest.generatedAtNanoseconds / 1e6;
}

export function assertShareAbsent(mod) {
  for (const name of shareExports) {
    assert.equal(name in mod, false, `Core WASM must omit Flow Share Bundle export ${name}`);
  }
}

export function qualifyShareBundles(mod, cases, oracle) {
  const encoder = new TextEncoder();
  let bundles = 0;
  let entries = 0;
  let manifests = 0;

  assert.equal(cases.length, oracle.length);
  for (let index = 0; index < cases.length; index++) {
    const spec = cases[index];
    const expected = oracle[index];
    assert.equal(expected.name, spec.name);

    const engine = makeEngine(mod, spec);
    const builder = new mod.ShareBundleBuilder();
    assert(builder instanceof mod.ShareBundleBuilder);
    configure(mod, builder, spec);

    if (expected.error !== undefined) {
      const before = Date.now();
      assert.throws(() => builder.scanAndBuild(engine, spec.sources), (error) => {
        assert(error instanceof Error);
        assert.equal(error.message, expected.error);
        for (const input of spec.sources) {
          assert.equal(error.message.includes(input.source), false, "Core error must not contain source text");
        }
        return true;
      });
      assert(Date.now() >= before);
      builder.free();
      engine.free();
      continue;
    }

    const before = Date.now();
    const bundle = builder.scanAndBuild(engine, spec.sources);
    const after = Date.now();
    assert(bundle instanceof mod.ShareBundle, `${spec.name}: bundle class`);
    bundles++;
    assert.equal(bundle.length, spec.sources.length);
    assert.equal(bundle.isEmpty(), spec.sources.length === 0);

    const manifest = bundle.manifest();
    assert(manifest instanceof mod.ShareManifestView);
    assert.equal(manifest.mode, mod.ShareModeKind[expected.manifest.mode]);
    assert.deepEqual(projectSummary(mod, manifest.summary()), expected.manifest.summary,
      `${spec.name}: manifest summary`);
    assert.deepEqual(projectSummary(mod, bundle.summary()), expected.manifest.summary,
      `${spec.name}: bundle summary`);
    const generatedAt = timestampMillis(manifest);
    assert(generatedAt >= before - 1000 && generatedAt <= after + 1000,
      `${spec.name}: Core timestamp outside the invocation window`);
    manifests++;

    const retained = [];
    for (let entryIndex = 0; entryIndex < spec.sources.length; entryIndex++) {
      const input = spec.sources[entryIndex];
      const output = expected.sources[entryIndex];
      const entry = bundle.entryAt(entryIndex);
      assert(entry instanceof mod.ShareBundleEntry);
      assert.equal(entry.id, output.id, `${spec.name}: ID preserved at ${entryIndex}`);
      assert.equal(entry.id, input.id);
      assert.equal(entry.content, output.content, `${spec.name}: Core content parity at ${entryIndex}`);
      assert.equal(encoder.encode(entry.content).length, encoder.encode(output.content).length);
      const second = bundle.entryAt(entryIndex);
      assert(second instanceof mod.ShareBundleEntry);
      assert.notEqual(second, entry, "each entry access must be independently owned");
      assert.equal(second.id, output.id);
      assert.equal(second.content, output.content);
      retained.push({ entry, second, input, output });
      entries++;
    }
    assert.throws(() => bundle.entryAt(spec.sources.length), `${spec.name}: entry index boundary`);
    assert.throws(() => bundle.entryAt(-1), `${spec.name}: negative entry index`);

    // Entries, manifest and summaries must remain valid after every owner dies.
    builder.free();
    engine.free();
    bundle.free();
    assert.equal(manifest.mode, mod.ShareModeKind[expected.manifest.mode]);
    assert.deepEqual(projectSummary(mod, manifest.summary()), expected.manifest.summary,
      `${spec.name}: manifest outlives bundle`);
    timestampMillis(manifest);
    for (const { entry, second, input, output } of retained) {
      assert.equal(entry.id, input.id);
      assert.equal(entry.content, output.content);
      assert.equal(second.id, output.id);
      assert.equal(second.content, output.content);
      second.free();
      entry.free();
    }
    manifest.free();
  }

  // Core's explicit mode requirement is preserved. The error carries no source.
  const missingRule = {
    builtins: false,
    rules: [{ kind: "literal", id: "literal", value: "SECRET", severity: "High" }],
  };
  const engine = makeEngine(mod, missingRule);
  const unconfigured = new mod.ShareBundleBuilder();
  const secretSource = "TOKEN=SECRET";
  assert.throws(() => unconfigured.scanAndBuild(engine, [{ id: "one", source: secretSource }]),
    (error) => {
      assert(error instanceof Error);
      assert.equal(error.message, "share bundle transformation mode was not configured");
      assert.equal(error.message.includes(secretSource), false);
      return true;
    });

  // Invalid key material fails before staging a mode; the builder remains usable.
  for (const bytes of [31, 33]) {
    const key = new Uint8Array(bytes);
    assert.throws(() => unconfigured.modePseudonymize(key, undefined, undefined));
    assert.throws(() => unconfigured.modeSynthesize(key, undefined));
  }
  assert.throws(() => unconfigured.scanAndBuild(engine, [{ id: "one", source: secretSource }]));
  unconfigured.modeRedact();
  const recovered = unconfigured.scanAndBuild(engine, [{ id: "one", source: secretSource }]);
  const recoveredEntry = recovered.entryAt(0);
  assert.equal(recoveredEntry.content, "TOKEN=[REDACTED]");
  recoveredEntry.free();
  recovered.free();
  unconfigured.free();

  // Last mode wins, as in Core's builder.
  const builder = new mod.ShareBundleBuilder();
  builder.modeSynthesize(new Uint8Array(32).fill(4), undefined);
  builder.modeRedact();
  const lastWins = builder.scanAndBuild(engine, [{ id: "one", source: secretSource }]);
  const lastWinsManifest = lastWins.manifest();
  const lastWinsEntry = lastWins.entryAt(0);
  assert.equal(lastWinsManifest.mode, mod.ShareModeKind.Redact);
  assert.equal(lastWinsEntry.content, "TOKEN=[REDACTED]");
  lastWinsManifest.free();
  lastWinsEntry.free();
  lastWins.free();
  builder.free();

  // Structural inputs fail before Core and never echo source text.
  const structural = new mod.ShareBundleBuilder();
  structural.modeRedact();
  for (const invalid of [null, undefined, "x", 1, true, {}, [], () => {}]) {
    assert.throws(() => structural.scanAndBuild(engine, [invalid]), `invalid share element: ${String(invalid)}`);
  }
  for (const invalid of [{}, { id: "only-id" }, { source: "only-source" },
    { id: 1, source: secretSource }, { id: "id", source: 1 },
    { id: null, source: secretSource }, { id: "id", source: null },
    { id: undefined, source: secretSource }, { id: "id", source: undefined }]) {
    assert.throws(() => structural.scanAndBuild(engine, [invalid]), (error) => {
      assert(error instanceof Error);
      assert.equal(error.message.includes(secretSource), false);
      return true;
    });
  }
  assert.throws(() => structural.scanAndBuild(engine, null));
  assert.throws(() => structural.scanAndBuild(engine, "not an array"));
  const empty = structural.scanAndBuild(engine, []);
  assert.equal(empty.length, 0);
  assert.equal(empty.isEmpty(), true);
  assert.deepEqual(projectSummary(mod, empty.summary()), {
    scannedSources: 0, scannedBytes: 0, reportsWithFindings: 0, reportsWithoutFindings: 0,
    totalFindings: 0, totalCandidates: 0, reportsWithCandidates: 0, hasCandidates: false,
    critical: 0, high: 0, medium: 0, low: 0, info: 0, isClean: true, hasCritical: false,
  });
  empty.free();

  // A builder may be reused; each invocation returns independent owned results.
  const inputs = [
    { id: "same", source: "é SECRET" },
    { id: "", source: "clean" },
  ];
  const first = structural.scanAndBuild(engine, inputs);
  const second = structural.scanAndBuild(engine, inputs);
  assert.notEqual(first, second);
  const firstEntry = first.entryAt(0);
  const secondEntry = second.entryAt(0);
  const firstManifest = first.manifest();
  const secondManifest = second.manifest();
  assert.notEqual(firstEntry, secondEntry);
  assert.notEqual(firstManifest, secondManifest);
  first.free();
  assert.equal(secondEntry.content, "é [REDACTED]");
  assert.equal(secondManifest.mode, mod.ShareModeKind.Redact);
  assert.equal(firstEntry.id, "same");
  const lateSummary = firstManifest.summary();
  assert.equal(lateSummary.scannedSources, 2);
  lateSummary.free();
  second.free();
  firstEntry.free();
  secondEntry.free();
  firstManifest.free();
  secondManifest.free();
  structural.free();
  engine.free();

  // Candidate-only input is review metadata, never transform authority.
  const candidateEngine = new mod.ScanEngine();
  const candidateBuilder = new mod.ShareBundleBuilder();
  candidateBuilder.modeRedact();
  const candidateSource = "ABCD-EFGH-IJKL-MNOP";
  const candidateBundle = candidateBuilder.scanAndBuild(candidateEngine, [
    { id: "review", source: candidateSource },
  ]);
  const candidateEntry = candidateBundle.entryAt(0);
  const candidateSummary = candidateBundle.summary();
  assert.equal(candidateEntry.content, candidateSource);
  assert.equal(candidateSummary.totalFindings, 0);
  assert.equal(candidateSummary.totalCandidates, 1);
  candidateEntry.free();
  candidateSummary.free();
  candidateBundle.free();
  candidateBuilder.free();
  candidateEngine.free();

  console.log(`${"flow"}: share bundles passed ${bundles} Core cases, ${entries} ordered entries, ${manifests} manifest/content comparisons, ownership, validation and repeated construction`);
}
