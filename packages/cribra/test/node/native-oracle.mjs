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
