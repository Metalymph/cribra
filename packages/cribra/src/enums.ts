/** Native numeric enum <-> public string union projection. Internal only. */

import type {
  CandidateEvidence,
  CandidateKind,
  Confidence,
  DetectionMode,
  Remediation,
  RuleKind,
  Severity,
} from "./types.js";

const SEVERITIES: readonly Severity[] = [
  "info",
  "low",
  "medium",
  "high",
  "critical",
];
const CONFIDENCES: readonly Confidence[] = ["low", "medium", "high"];
const DETECTION_MODES: readonly DetectionMode[] = [
  "matcher_only",
  "deterministic",
  "contextual",
];
const RULE_KINDS: readonly RuleKind[] = ["literal", "prefix", "suffix", "pattern"];
const CANDIDATE_KINDS: readonly CandidateKind[] = [
  "recovery_like_code",
  "unknown",
];
const CANDIDATE_EVIDENCE: readonly CandidateEvidence[] = ["structural", "unknown"];

/** None is represented by null, so slot 0 is null. */
const REMEDIATIONS: readonly (Remediation | null)[] = [
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

const SEVERITY_INDEX: ReadonlyMap<Severity, number> = new Map(
  SEVERITIES.map((value, index) => [value, index]),
);

const REMEDIATION_INDEX: ReadonlyMap<Remediation, number> = new Map(
  REMEDIATIONS.filter((value): value is Remediation => value !== null).map(
    (value, index) => [value, index + 1],
  ),
);

function indexed<T>(table: readonly T[], value: number, field: string): T {
  if (!Number.isSafeInteger(value) || value < 0 || value >= table.length) {
    throw new Error(`Unknown native ${field} value`);
  }
  return table[value] as T;
}

export function severityFromNative(value: number): Severity {
  return indexed(SEVERITIES, value, "severity");
}

export function confidenceFromNative(value: number): Confidence {
  return indexed(CONFIDENCES, value, "confidence");
}

export function detectionModeFromNative(value: number): DetectionMode {
  return indexed(DETECTION_MODES, value, "detection mode");
}

export function ruleKindFromNative(value: number): RuleKind {
  return indexed(RULE_KINDS, value, "rule kind");
}

export function candidateKindFromNative(value: number): CandidateKind {
  return indexed(CANDIDATE_KINDS, value, "candidate kind");
}

export function candidateEvidenceFromNative(value: number): CandidateEvidence {
  return indexed(CANDIDATE_EVIDENCE, value, "candidate evidence");
}

export function remediationFromNative(value: number): Remediation | null {
  return indexed(REMEDIATIONS, value, "remediation");
}

export function isSeverity(value: unknown): value is Severity {
  return typeof value === "string" && SEVERITY_INDEX.has(value as Severity);
}

export function isRemediation(value: unknown): value is Remediation {
  return typeof value === "string" && REMEDIATION_INDEX.has(value as Remediation);
}

export function severityToNative(value: Severity): number {
  const index = SEVERITY_INDEX.get(value);
  if (index === undefined) {
    throw new Error("Unknown severity");
  }
  return index;
}

export function remediationToNative(value: Remediation): number {
  const index = REMEDIATION_INDEX.get(value);
  if (index === undefined) {
    throw new Error("Unknown remediation");
  }
  return index;
}

export const BACKEND_KINDS: readonly ("wasm" | "native")[] = ["wasm"];
export const PROFILE_KINDS: readonly ("core" | "flow")[] = ["core", "flow"];

export function backendFromNative(value: number): "wasm" | "native" {
  return indexed(BACKEND_KINDS, value, "backend");
}

export function profileFromNative(value: number): "core" | "flow" {
  return indexed(PROFILE_KINDS, value, "profile");
}
