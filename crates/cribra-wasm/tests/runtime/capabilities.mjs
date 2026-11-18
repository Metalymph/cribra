// Adapter identity, capability manifest, and operation-level qualification.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { makeEngine } from "./stream.mjs";
import { assertShareAbsent } from "./share_bundle.mjs";

const FIELDS = [
  "backend",
  "profile",
  "singleSourceScan",
  "batchScan",
  "streaming",
  "incremental",
  "redaction",
  "templating",
  "pseudonymization",
  "synthesis",
  "shareBundle",
  "ruleMetadata",
  "explanations",
  "parallel",
];

function packageVersion(source) {
  const match = source.match(/^version = "([^"]+)"$/m);
  assert(match, "adapter package version must be readable from Cargo.toml");
  return match[1];
}

function copyManifest(manifest) {
  return Object.fromEntries(FIELDS.map((field) => [field, manifest[field]]));
}

export async function qualifyCapabilities(mod, variant, directory) {
  assert.equal(typeof mod.wasmAdapterVersion, "function");
  assert.equal(mod.wasmAdapterVersion.length, 0);
  const version = mod.wasmAdapterVersion();
  assert.equal(typeof version, "string");
  assert.notEqual(version.length, 0);
  const manifestSource = await readFile(new URL("../../Cargo.toml", import.meta.url), "utf8");
  assert.equal(version, packageVersion(manifestSource), "adapter version must match the adapter package");

  const declarations = await readFile(resolve(directory, "cribra.d.ts"), "utf8");
  assert.match(declarations, /export function wasmAdapterVersion\(\): string;/);
  assert.match(declarations, /export function capabilityManifest\(\): CapabilityManifest;/);

  assert.equal(typeof mod.capabilityManifest, "function");
  assert.equal(mod.capabilityManifest.length, 0);
  const nativeManifest = mod.capabilityManifest();
  let manifest;
  try {
    const prototype = Object.getOwnPropertyDescriptors(Object.getPrototypeOf(nativeManifest));
    assert.deepEqual(
      Object.keys(prototype).filter((key) => prototype[key].get).sort(),
      [...FIELDS].sort(),
      "manifest getter shape must be stable",
    );
    assert.equal(typeof nativeManifest.free, "function");
    // The wrapper must copy every getter and free the native manifest, even
    // when an error occurs. The copied plain object is safe after free().
    manifest = copyManifest(nativeManifest);
  } finally {
    nativeManifest.free();
  }
  assert.deepEqual(Object.keys(manifest), FIELDS);
  for (const field of FIELDS.slice(2)) {
    assert.equal(typeof manifest[field], "boolean", `${field} must be boolean`);
  }
  assert.equal(mod.BackendKind[manifest.backend], "Wasm");
  assert.equal(mod.ProfileKind[manifest.profile], variant === "flow" ? "Flow" : "Core");

  const source = "CAPABILITY_SECRET";
  const sourceBytes = new TextEncoder().encode(source).length;
  const engine = makeEngine(mod, {
    rules: [{ id: "capability.secret", kind: "literal", value: source, severity: "High" }],
  });
  const expectFinding = (result) => {
    assert.equal(result.sourceBytes, sourceBytes);
    assert.equal(result.findingCount(), 1);
    assert.equal(result.candidateCount(), 0);
    const finding = result.findingAt(0);
    try {
      assert.equal(finding.ruleId, "capability.secret");
      assert.deepEqual(
        [finding.start, finding.end, finding.line, finding.column],
        [0, sourceBytes, 1, 1],
      );
      assert.equal(finding.severity, mod.FindingSeverity.High);
    } finally {
      finding.free();
    }
  };

  try {
    assert.equal(manifest.singleSourceScan, typeof engine.scan === "function");
    const scanned = engine.scan(source);
    try {
      expectFinding(scanned);

      assert.equal(manifest.batchScan, typeof engine.scanBatch === "function");
      const batch = engine.scanBatch([
        { id: "first", source },
        { id: "second", source: "clean" },
      ]);
      try {
        assert.equal(batch.length, 2);
        const first = batch.entryAt(0);
        const second = batch.entryAt(1);
        try {
          const firstResult = first.result;
          const secondResult = second.result;
          try {
            assert.equal(first.id, "first");
            assert.equal(second.id, "second");
            assert.equal(firstResult.findingCount(), 1);
            assert.equal(secondResult.findingCount(), 0);
          } finally {
            firstResult.free();
            secondResult.free();
          }
        } finally {
          first.free();
          second.free();
        }
        const summary = batch.summary();
        try {
          assert.equal(summary.scannedSources, 2);
          assert.equal(summary.totalFindings, 1);
        } finally {
          summary.free();
        }
      } finally {
        batch.free();
      }

      assert.equal(manifest.streaming, typeof engine.stream === "function");
      const stream = engine.stream();
      const streamed = (() => {
        try {
          const bytes = new TextEncoder().encode(source);
          stream.push(bytes.subarray(0, 4));
          stream.push(bytes.subarray(4));
          return stream.finish();
        } finally {
          stream.free();
        }
      })();
      try {
        expectFinding(streamed);
      } finally {
        streamed.free();
      }

      assert.equal(manifest.incremental, typeof engine.incremental === "function");
      const session = engine.incremental(source);
      try {
        const initial = session.snapshot();
        try {
          assert.equal(initial.revision, 0n);
          const initialResult = initial.result;
          try {
            expectFinding(initialResult);
          } finally {
            initialResult.free();
          }
        } finally {
          initial.free();
        }
        assert.equal(session.applyEdit(0n, 0, source.length, "SAFE"), 1n);
        const edited = session.snapshot();
        try {
          assert.equal(edited.revision, 1n);
          const editedResult = edited.result;
          try {
            assert.equal(editedResult.findingCount(), 0);
          } finally {
            editedResult.free();
          }
        } finally {
          edited.free();
        }
      } finally {
        session.free();
      }

      assert.equal(manifest.ruleMetadata, typeof engine.activeRules === "function");
      const rules = engine.activeRules();
      try {
        assert.equal(rules.length, 1);
        assert.equal(rules[0].ruleId, "capability.secret");
        assert.equal(rules[0].kind, mod.RuleKind.Literal);
      } finally {
        for (const rule of rules) rule.free();
      }

      assert.equal(manifest.explanations, typeof engine.explainFinding === "function");
      const explanation = engine.explainFinding(scanned, 0);
      try {
        assert.equal(explanation.kind, mod.ExplanationKind.Classified);
        assert.equal(explanation.detectionMode, mod.DetectionModeKind.MatcherOnly);
      } finally {
        explanation.free();
      }

      const transforms = typeof scanned.redact === "function";
      assert.equal(manifest.redaction, transforms && typeof scanned.redactWith === "function");
      assert.equal(
        manifest.templating,
        transforms && typeof scanned.template === "function" && typeof scanned.templateWith === "function",
      );
      assert.equal(manifest.pseudonymization, transforms && typeof scanned.pseudonymize === "function");
      assert.equal(manifest.synthesis, transforms && typeof scanned.synthesize === "function");
      assert.equal(manifest.shareBundle, typeof mod.ShareBundleBuilder === "function");

      if (transforms) {
        assert.equal(scanned.redact(source), "[REDACTED]");
        assert.equal(scanned.redactWith(source, "<hidden>"), "<hidden>");
        const templated = scanned.template(source);
        assert(!templated.includes(source));
        assert(templated.startsWith("<"));
        assert.equal(
          scanned.templateWith(source, "SAFE", true),
          scanned.templateWith(source, "SAFE", true),
        );
        const key = new Uint8Array(32).fill(7);
        const pseudonym = scanned.pseudonymize(source, key, "pseudo_", 16);
        assert(!pseudonym.includes(source));
        assert(pseudonym.startsWith("pseudo_"));
        assert.equal(scanned.pseudonymize(source, key, "pseudo_", 16), pseudonym);
        const synthetic = scanned.synthesize(source, key, "synthetic");
        assert(!synthetic.includes(source));
        assert(synthetic.startsWith("synthetic"));
        assert.equal(scanned.synthesize(source, key, "synthetic"), synthetic);
      } else {
        for (const method of [
          "redact",
          "redactWith",
          "template",
          "templateWith",
          "pseudonymize",
          "synthesize",
        ]) {
          assert.equal(method in scanned, false, `Core must not expose ${method}`);
        }
      }

      if (manifest.shareBundle) {
        const builder = new mod.ShareBundleBuilder();
        try {
          builder.modeRedact();
          const bundle = builder.scanAndBuild(engine, [{ id: "share", source }]);
          try {
            assert.equal(bundle.length, 1);
            const entry = bundle.entryAt(0);
            try {
              assert.equal(entry.id, "share");
              assert.equal(entry.content, "[REDACTED]");
            } finally {
              entry.free();
            }
            const shareManifest = bundle.manifest();
            try {
              assert.equal(shareManifest.mode, mod.ShareModeKind.Redact);
            } finally {
              shareManifest.free();
            }
            const summary = bundle.summary();
            try {
              assert.equal(summary.totalFindings, 1);
            } finally {
              summary.free();
            }
          } finally {
            bundle.free();
          }
        } finally {
          builder.free();
        }
      } else {
        assertShareAbsent(mod);
      }

      assert.equal(manifest.parallel, false);
      assert.equal(typeof engine.scanParallel, "undefined");
      assert.equal(typeof mod.ScanEngine.prototype.scanParallel, "undefined");
    } finally {
      scanned.free();
    }
  } finally {
    engine.free();
  }

  // `streaming: true` reports that the operation is exported. Admission
  // remains a per-ruleset decision made by Core.
  const builder = new mod.ScanEngineBuilder(false);
  builder.addSuffix("capability.unsupported", "!", mod.FindingSeverity.High);
  const unsupported = builder.build();
  try {
    let error;
    try {
      const session = unsupported.stream();
      session.free();
    } catch (value) {
      error = value;
    }
    assert(error instanceof mod.StreamError);
    try {
      assert.equal(error.code, mod.StreamErrorCode.UnsupportedRuleSet);
    } finally {
      error.free();
    }
  } finally {
    unsupported.free();
  }

  console.log(
    `${variant}: adapter version ${version}, ${FIELDS.length} manifest fields, and supported operations qualified`,
  );
}
