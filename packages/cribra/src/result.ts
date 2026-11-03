/** Native view -> plain DTO projection. Every owned view is freed by the caller. */

import { ExplanationKind } from "./wasm/cribra.js";
import type {
  ExplanationView,
  FindingView,
  RuleMetadataView,
  ScanEngine,
  ScanResult as NativeScanResult,
  ScanSummaryView,
} from "./wasm/cribra.js";
import {
  candidateEvidenceFromNative,
  candidateKindFromNative,
  confidenceFromNative,
  detectionModeFromNative,
  remediationFromNative,
  ruleKindFromNative,
  severityFromNative,
} from "./enums.js";
import type {
  BatchEntry,
  Candidate,
  Explanation,
  Finding,
  RuleMetadata,
  ScanResult,
  ScanSummary,
} from "./types.js";

function span(start: number, end: number): { start: number; end: number } {
  return Object.freeze({ start, end });
}

function projectExplanation(view: ExplanationView): Explanation {
  try {
    if (view.kind === ExplanationKind.Classified) {
      return Object.freeze({
        kind: "classified" as const,
        detectionMode: detectionModeFromNative(view.detectionMode as number),
      });
    }
    if (view.kind === ExplanationKind.Ambiguous) {
      return Object.freeze({
        kind: "ambiguous" as const,
        evidence: candidateEvidenceFromNative(view.candidateEvidence as number),
      });
    }
    return Object.freeze({ kind: "unknown" as const });
  } finally {
    view.free();
  }
}

function projectFinding(view: FindingView, explanation?: Explanation): Finding {
  return Object.freeze({
    ruleId: view.ruleId,
    span: span(view.start, view.end),
    line: view.line,
    column: view.column,
    severity: severityFromNative(view.severity),
    confidence: confidenceFromNative(view.confidence),
    remediation: remediationFromNative(view.remediation),
    ...(explanation === undefined ? {} : { explanation }),
  });
}

function readCandidate(
  native: NativeScanResult,
  index: number,
): Omit<Candidate, "explanation"> {
  const view = native.candidateAt(index);
  try {
    return {
      kind: candidateKindFromNative(view.kind),
      evidence: candidateEvidenceFromNative(view.evidence),
      span: span(view.start, view.end),
      line: view.line,
      column: view.column,
    };
  } finally {
    view.free();
  }
}

export function projectScanResult(
  engine: ScanEngine,
  native: NativeScanResult,
  includeExplanations: boolean,
): ScanResult {
  const findingCount = native.findingCount();
  const candidateCount = native.candidateCount();

  const findings: Finding[] = [];
  for (let index = 0; index < findingCount; index += 1) {
    const view = native.findingAt(index);
    try {
      let explanation: Explanation | undefined;
      if (includeExplanations) {
        explanation = projectExplanation(engine.explainFinding(native, index));
      }
      findings.push(projectFinding(view, explanation));
    } finally {
      view.free();
    }
  }

  const candidates: Candidate[] = [];
  for (let index = 0; index < candidateCount; index += 1) {
    const base = readCandidate(native, index);
    const explanation = projectExplanation(
      native.candidateExplanationAt(index),
    );
    candidates.push(Object.freeze({ ...base, explanation }));
  }

  return Object.freeze({
    sourceBytes: native.sourceBytes,
    findings: Object.freeze(findings),
    candidates: Object.freeze(candidates),
    findingCount,
    candidateCount,
    needsReview: native.needsReview(),
    hasCritical: native.hasCritical(),
  });
}

export function projectBatchEntry(
  id: string,
  sourceBytes: number,
  result: ScanResult,
): BatchEntry {
  return Object.freeze({ id, sourceBytes, result });
}

export function projectSummary(view: ScanSummaryView): ScanSummary {
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

export function projectRuleMetadata(view: RuleMetadataView): RuleMetadata {
  try {
    return Object.freeze({
      ruleId: view.ruleId,
      kind: ruleKindFromNative(view.kind),
      detectionMode: detectionModeFromNative(view.detectionMode),
      severity: severityFromNative(view.severity),
      remediation: remediationFromNative(view.remediation),
    });
  } finally {
    view.free();
  }
}
