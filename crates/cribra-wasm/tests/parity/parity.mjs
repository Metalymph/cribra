import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

const root = process.cwd();
const oraclePath = resolve(root, "target/wasm-parity/oracle.json");
const gluePath = resolve(root, "target/wasm-production/cribra.js");
const wasmPath = resolve(root, "target/wasm-production/cribra_bg.wasm");

const oracle = JSON.parse(await readFile(oraclePath, "utf8"));
const mod = await import(pathToFileURL(gluePath).href);
const wasmBytes = await readFile(wasmPath);

await mod.default({ module_or_path: wasmBytes });

const enumName = (table, value) => {
  for (const [name, numeric] of Object.entries(table)) {
    if (numeric === value) {
      return name;
    }
  }

  return "Unknown";
};

const assertEqual = (actual, expected, context) => {
  if (actual !== expected) {
    throw new Error(
      `${context}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
    );
  }
};

const assertDeepEqual = (actual, expected, context) => {
  const left = JSON.stringify(actual);
  const right = JSON.stringify(expected);

  if (left !== right) {
    throw new Error(`${context}:\nexpected ${right}\nactual   ${left}`);
  }
};

function buildCanonicalScanner() {
  const builder = new mod.ScanEngineBuilder(false);

  builder.addPrefix(
    "demo.api-key",
    "demo_api_",
    mod.FindingSeverity.Critical,
  );

  builder.addPattern(
    "demo.password",
    "demo-pass-[A-Za-z0-9_\\-\\p{L}]+",
    mod.FindingSeverity.High,
  );

  builder.addLiteral(
    "demo.private-key",
    "DEMO_PRIVATE_KEY_MATERIAL",
    mod.FindingSeverity.Critical,
  );

  builder.addLiteral(
    "demo.secret",
    "DEMO_SECRET_ALPHA",
    mod.FindingSeverity.High,
  );

  return builder.build();
}

function buildScanner(kind) {
  switch (kind) {
    case "canonical_custom":
      return buildCanonicalScanner();

    case "default_builtins":
      return new mod.ScanEngine();

    default:
      throw new Error(`unknown scanner kind: ${kind}`);
  }
}

function projectExplanation(view) {
  try {
    const kind = enumName(mod.ExplanationKind, view.kind);

    if (kind === "Classified") {
      return {
        kind: "classified",
        detection_mode: enumName(
          mod.DetectionModeKind,
          view.detectionMode,
        ),
      };
    }

    if (kind === "Ambiguous") {
      return {
        kind: "ambiguous",
        evidence: enumName(
          mod.CandidateEvidenceKind,
          view.candidateEvidence,
        ),
      };
    }

    return {
      kind: "unknown",
    };
  } finally {
    view.free();
  }
}

function projectFinding(engine, result, index) {
  const view = result.findingAt(index);

  try {
    return {
      rule_id: view.ruleId,
      start: view.start,
      end: view.end,
      line: view.line,
      column: view.column,
      severity: enumName(
        mod.FindingSeverity,
        view.severity,
      ),
      confidence: enumName(
        mod.FindingConfidence,
        view.confidence,
      ),
      remediation: (() => {
        const name = enumName(
          mod.RemediationKind,
          view.remediation,
        );

        return name === "None" ? null : name;
      })(),
      explanation: projectExplanation(
        engine.explainFinding(result, index),
      ),
    };
  } finally {
    view.free();
  }
}

function projectCandidate(result, index) {
  const view = result.candidateAt(index);

  try {
    return {
      start: view.start,
      end: view.end,
      line: view.line,
      column: view.column,
      kind: enumName(
        mod.CandidateKind,
        view.kind,
      ),
      evidence: enumName(
        mod.CandidateEvidenceKind,
        view.evidence,
      ),
      explanation: projectExplanation(
        result.candidateExplanationAt(index),
      ),
    };
  } finally {
    view.free();
  }
}

function compareCase(expected) {
  const engine = buildScanner(expected.scanner);
  const result = engine.scan(expected.source);

  try {
    const prefix = expected.name;

    assertEqual(
      result.sourceBytes,
      expected.source_bytes,
      `${prefix}.source_bytes`,
    );

    assertEqual(
      result.findingCount(),
      expected.finding_count,
      `${prefix}.finding_count`,
    );

    assertEqual(
      result.candidateCount(),
      expected.candidate_count,
      `${prefix}.candidate_count`,
    );

    assertEqual(
      result.needsReview(),
      expected.needs_review,
      `${prefix}.needs_review`,
    );

    assertEqual(
      result.hasCritical(),
      expected.has_critical,
      `${prefix}.has_critical`,
    );

    const findings = Array.from(
      { length: result.findingCount() },
      (_, index) => projectFinding(
        engine,
        result,
        index,
      ),
    );

    const candidates = Array.from(
      { length: result.candidateCount() },
      (_, index) => projectCandidate(
        result,
        index,
      ),
    );

    assertDeepEqual(
      findings,
      expected.findings,
      `${prefix}.findings`,
    );

    assertDeepEqual(
      candidates,
      expected.candidates,
      `${prefix}.candidates`,
    );

    const pseudonymKey = new Uint8Array(32).fill(0x31);
    const synthesisKey = new Uint8Array(32).fill(0x53);

    const transforms = {
      redacted: result.redact(
        expected.source,
      ),
      template: result.template(
        expected.source,
      ),
      pseudonymized: result.pseudonymize(
        expected.source,
        pseudonymKey,
        "cribra_pseudo_",
        16,
      ),
      synthesized: result.synthesize(
        expected.source,
        synthesisKey,
        "cribra_synthetic",
      ),
    };

    assertDeepEqual(
      transforms,
      expected.transforms,
      `${prefix}.transforms`,
    );
  } finally {
    result.free();
    engine.free();
  }
}

if (oracle.schema !== 1) {
  throw new Error(
    `unsupported parity oracle schema: ${oracle.schema}`,
  );
}

for (const testCase of oracle.cases) {
  compareCase(testCase);

  console.log(
    `parity ok: ${testCase.name}`,
  );
}

console.log(
  `cribra wasm semantic parity: ok (${oracle.cases.length} cases)`,
);