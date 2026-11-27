/**
 * Pure Guard v1 policy evaluation.
 *
 * The policy answers one question: may this snapshot be submitted normally, or
 * must the user review it first? It never transforms, scans, submits, or
 * persists anything.
 */

import {
  DEFAULT_TRANSFORMATION_PREFERENCE,
  type ScanSummary,
  type TransformationPreference,
} from "./types.js";

export type ReviewReason =
  | "findings"
  | "candidates"
  | "findings_and_candidates"
  | "unsupported_coverage";

export interface CleanDecision {
  readonly kind: "clean";
  readonly snapshotId: string;
}

export interface ReviewDecision {
  readonly kind: "review";
  readonly snapshotId: string;
  readonly reason: ReviewReason;
  /** A transformation may be offered after the user chooses to review. */
  readonly canTransform: boolean;
  /** The user may explicitly approve sending the original. */
  readonly canApproveOriginal: boolean;
  /** The transformation shown as the primary choice. Never consent by itself. */
  readonly primaryTransformation: TransformationPreference;
}

export type PolicyDecision = CleanDecision | ReviewDecision;

function review(
  summary: ScanSummary,
  reason: ReviewReason,
  canTransform: boolean,
  canApproveOriginal: boolean,
  preference: TransformationPreference,
): ReviewDecision {
  return {
    kind: "review",
    snapshotId: summary.snapshotId,
    reason,
    canTransform,
    canApproveOriginal,
    primaryTransformation: preference,
  };
}

/**
 * Evaluates the v1 policy for a validated summary.
 *
 * Semantics:
 * - complete coverage with zero findings and zero candidates: clean;
 * - any finding at any severity: review, transformation and explicit original
 *   approval are both available;
 * - any candidate: review, and transformation-and-send is unavailable;
 * - unsupported or incomplete coverage: review, distinct from clean and from a
 *   complete scan with findings. Neither transformation nor original approval is
 *   available because the scanner could not establish complete coverage. Review
 *   and edit remain available, and cancellation remains available.
 */
export function evaluatePolicy(
  summary: ScanSummary,
  preference: TransformationPreference = DEFAULT_TRANSFORMATION_PREFERENCE,
): PolicyDecision {
  if (summary.coverage === "unsupported") {
    return review(summary, "unsupported_coverage", false, false, preference);
  }

  const hasFindings = summary.findingCount > 0;
  const hasCandidates = summary.candidateCount > 0;

  if (!hasFindings && !hasCandidates) {
    return { kind: "clean", snapshotId: summary.snapshotId };
  }

  if (hasFindings && hasCandidates) {
    return review(summary, "findings_and_candidates", false, true, preference);
  }

  if (hasCandidates) {
    return review(summary, "candidates", false, true, preference);
  }

  return review(summary, "findings", true, true, preference);
}
