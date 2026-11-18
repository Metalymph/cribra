/* Independent projections over the generated bindings, used only as test oracles. */

import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { distDir } from "../helpers.mjs";

const SEVERITIES = ["info", "low", "medium", "high", "critical"];
const CONFIDENCES = ["low", "medium", "high"];
const DETECTION_MODES = ["matcher_only", "deterministic", "contextual"];
const CANDIDATE_KINDS = ["recovery_like_code", "unknown"];
const CANDIDATE_EVIDENCE = ["structural", "unknown"];
const REMEDIATIONS = [
  null,
  "revoke_and_rotate_credential",
  "rotate_credential",
  "rotate_password",
  "replace_private_key",
  "remove_sensitive_value",
  "review_sensitive_hash",
  "review_password_verifier",
  "unknown",
];

const SEVERITY_KEYS = {
  info: "Info",
  low: "Low",
  medium: "Medium",
  high: "High",
  critical: "Critical",
};
const REMEDIATION_KEYS = {
  revoke_and_rotate_credential: "RevokeAndRotateCredential",
  rotate_credential: "RotateCredential",
  rotate_password: "RotatePassword",
  replace_private_key: "ReplacePrivateKey",
  remove_sensitive_value: "RemoveSensitiveValue",
  review_sensitive_hash: "ReviewSensitiveHash",
  review_password_verifier: "ReviewPasswordVerifier",
  unknown: "Unknown",
};
const METHODS = {
  literal: "addLiteral",
  prefix: "addPrefix",
  suffix: "addSuffix",
  pattern: "addPattern",
};

export async function loadNative() {
  return import(
    pathToFileURL(resolve(distDir, "wasm/cribra.js")).href
  );
}

export function buildNativeEngine(native, rules) {
  const builder = new native.ScanEngineBuilder(false);
  try {
    for (const rule of rules) {
      const method = METHODS[rule.kind];
      const payload = rule.kind === "pattern" ? rule.pattern : rule.value;
      const severity = native.FindingSeverity[SEVERITY_KEYS[rule.severity]];
      const remediation =
        rule.remediation === undefined || rule.remediation === null
          ? undefined
          : native.RemediationKind[REMEDIATION_KEYS[rule.remediation]];
      if (remediation === undefined) {
        builder[method](rule.id, payload, severity);
      } else {
        builder[`${method}WithRemediation`](
          rule.id,
          payload,
          severity,
          remediation,
        );
      }
    }
  } catch (error) {
    builder.free();
    throw error;
  }
  return builder.build();
}

function explanation(native, view) {
  try {
    if (view.kind === native.ExplanationKind.Classified) {
      return Object.freeze({
        kind: "classified",
        detectionMode: DETECTION_MODES[view.detectionMode],
      });
    }
    if (view.kind === native.ExplanationKind.Ambiguous) {
      return Object.freeze({
        kind: "ambiguous",
        evidence: CANDIDATE_EVIDENCE[view.candidateEvidence],
      });
    }
    return Object.freeze({ kind: "unknown" });
  } finally {
    view.free();
  }
}

export function nativeReport(native, result) {
  const findingCount = result.findingCount();
  const candidateCount = result.candidateCount();

  const findings = [];
  for (let index = 0; index < findingCount; index += 1) {
    const view = result.findingAt(index);
    try {
      findings.push(
        Object.freeze({
          ruleId: view.ruleId,
          span: Object.freeze({ start: view.start, end: view.end }),
          line: view.line,
          column: view.column,
          severity: SEVERITIES[view.severity],
          confidence: CONFIDENCES[view.confidence],
          remediation: REMEDIATIONS[view.remediation],
        }),
      );
    } finally {
      view.free();
    }
  }

  const candidates = [];
  for (let index = 0; index < candidateCount; index += 1) {
    const view = result.candidateAt(index);
    try {
      candidates.push(
        Object.freeze({
          kind: CANDIDATE_KINDS[view.kind],
          evidence: CANDIDATE_EVIDENCE[view.evidence],
          span: Object.freeze({ start: view.start, end: view.end }),
          line: view.line,
          column: view.column,
          explanation: explanation(
            native,
            result.candidateExplanationAt(index),
          ),
        }),
      );
    } finally {
      view.free();
    }
  }

  return Object.freeze({
    sourceBytes: result.sourceBytes,
    findings: Object.freeze(findings),
    candidates: Object.freeze(candidates),
    findingCount,
    candidateCount,
    needsReview: result.needsReview(),
    hasCritical: result.hasCritical(),
  });
}

export function nativeScan(native, engine, source) {
  const result = engine.scan(source);
  try {
    return nativeReport(native, result);
  } finally {
    result.free();
  }
}

export function nativeStream(native, engine, chunks) {
  const session = engine.stream();
  try {
    for (const chunk of chunks) {
      session.push(Uint8Array.from(chunk));
    }
    const result = session.finish();
    try {
      return nativeReport(native, result);
    } finally {
      result.free();
    }
  } finally {
    session.free();
  }
}

export function nativeSnapshot(native, session) {
  const snapshot = session.snapshot();
  try {
    const result = snapshot.result;
    try {
      return Object.freeze({
        revision: snapshot.revision,
        result: nativeReport(native, result),
      });
    } finally {
      result.free();
    }
  } finally {
    snapshot.free();
  }
}

const TRANSFORM_ERROR_NAMES = [
  "transform_invalid_span",
  "transform_invalid_utf8_boundary",
  "transform_overlapping_spans",
  "invalid_key_length",
  "transform_unknown",
];

/** Runs one Flow transformation on a fresh native scan, independent of the wrapper. */
export function nativeTransform(native, engine, source, request) {
  const result = engine.scan(source);
  try {
    switch (request.kind) {
      case "redact":
        return Object.freeze({
          output:
            request.replacement === undefined
              ? result.redact(source)
              : result.redactWith(source, request.replacement),
        });
      case "template":
        return Object.freeze({
          output: result.templateWith(
            source,
            request.namespace ?? "CRIBRA",
            request.numbered ?? false,
          ),
        });
      case "pseudonymize":
        return Object.freeze({
          output: result.pseudonymize(
            source,
            request.key,
            request.prefix ?? "cribra_pseudo_",
            request.digestBytes ?? 16,
          ),
        });
      case "synthesize":
        return Object.freeze({
          output: result.synthesize(
            source,
            request.key,
            request.marker ?? "cribra_synthetic",
          ),
        });
      default:
        throw new Error(`unknown transform oracle mode: ${request.kind}`);
    }
  } catch (error) {
    if (error && typeof error.free === "function" && typeof error.code === "number") {
      try {
        return Object.freeze({
          errorCode: TRANSFORM_ERROR_NAMES[error.code] ?? "transform_unknown",
        });
      } finally {
        error.free();
      }
    }
    throw error;
  } finally {
    result.free();
  }
}

const SHARE_MODE_NAMES = [
  "redact",
  "template",
  "pseudonymize",
  "synthesize",
  "unknown",
];

/** Projects one owned native summary view and frees it. */
export function nativeSummary(view) {
  try {
    return Object.freeze({
      scannedSources: view.scannedSources,
      scannedBytes: view.scannedBytes,
      reportsWithFindings: view.reportsWithFindings,
      reportsWithoutFindings: view.reportsWithoutFindings,
      totalFindings: view.totalFindings,
      totalCandidates: view.totalCandidates,
      reportsWithCandidates: view.reportsWithCandidates,
      hasCandidates: view.hasCandidates,
      critical: view.critical,
      high: view.high,
      medium: view.medium,
      low: view.low,
      info: view.info,
      isClean: view.isClean,
      hasCritical: view.hasCritical,
    });
  } finally {
    view.free();
  }
}

function configureNativeShareMode(native, builder, mode) {
  switch (mode.kind) {
    case "redact":
      builder.modeRedact();
      return;
    case "template":
      builder.modeTemplate();
      return;
    case "pseudonymize":
      builder.modePseudonymize(mode.key, mode.prefix, mode.digestBytes);
      return;
    case "synthesize":
      builder.modeSynthesize(mode.key, mode.marker);
      return;
    default:
      throw new Error(`unknown share oracle mode: ${mode.kind}`);
  }
}

/** Builds one Flow Share Bundle directly through the generated builder. */
export function nativeShareBundle(
  native,
  rules,
  sources,
  mode,
  generatedAtMillis,
) {
  const engine = buildNativeEngine(native, rules);
  const builder = new native.ShareBundleBuilder();
  try {
    configureNativeShareMode(native, builder, mode);
    const bundle =
      generatedAtMillis === undefined
        ? builder.scanAndBuild(engine, sources)
        : builder.scanAndBuildWithGeneratedAt(
            engine,
            sources,
            generatedAtMillis,
          );
    try {
      const entries = [];
      for (let index = 0; index < bundle.length; index += 1) {
        const entry = bundle.entryAt(index);
        try {
          entries.push(
            Object.freeze({ id: entry.id, content: entry.content }),
          );
        } finally {
          entry.free();
        }
      }
      const manifest = bundle.manifest();
      let projectedManifest;
      try {
        const seconds = manifest.generatedAtSeconds;
        if (seconds > BigInt(Number.MAX_SAFE_INTEGER)) {
          throw new Error("native manifest seconds exceed safe integer range");
        }
        projectedManifest = Object.freeze({
          mode: SHARE_MODE_NAMES[manifest.mode] ?? "unknown",
          generatedAtSeconds: Number(seconds),
          generatedAtNanoseconds: manifest.generatedAtNanoseconds,
          summary: nativeSummary(manifest.summary()),
        });
      } finally {
        manifest.free();
      }
      return Object.freeze({
        entries: Object.freeze(entries),
        manifest: projectedManifest,
        summary: nativeSummary(bundle.summary()),
      });
    } finally {
      bundle.free();
    }
  } finally {
    builder.free();
    engine.free();
  }
}
