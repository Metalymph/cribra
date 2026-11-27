/**
 * Deterministic unit tests for the pure Guard policy and state machine.
 *
 * These tests use only Node's built-in test runner and assertions. They do not
 * start Chromium, load WASM, or touch any browser API.
 */

import assert from "node:assert/strict";
import { describe, test } from "node:test";

import type { ScanResult } from "../cribra.js";
import {
  DEFAULT_TRANSFORMATION_PREFERENCE,
  GUARD_SEVERITIES,
  evaluatePolicy,
  initialGuardState,
  isValidTransactionIdentity,
  parseScanSummary,
  summarizeScan,
  transition,
} from "./index.js";
import type {
  FinalSnapshotVerification,
  GuardEvent,
  GuardPhase,
  GuardSeverity,
  GuardState,
  OriginalApprovalConfirmation,
  ReviewReason,
  ScanCoverage,
  ScanFailedEvent,
  ScanFailureReason,
  ScanStage,
  ScanSucceededEvent,
  ScanSummary,
  SubmissionOutcomeUnknownReason,
  TransactionIdentity,
  TransformationPreference,
  TransformFailedEvent,
  TransformSucceededEvent,
  TransitionRejection,
  TransitionResult,
} from "./index.js";

const SUBMISSION_ID = "submission-1";
const SESSION_ID = "session-1";
const SNAPSHOT_ID = "snapshot-1";
const TRANSFORMED_SNAPSHOT_ID = "snapshot-2";
const OTHER_SNAPSHOT_ID = "snapshot-other";

function transaction(
  overrides: Partial<TransactionIdentity> = {},
): TransactionIdentity {
  return {
    submissionId: SUBMISSION_ID,
    sessionId: SESSION_ID,
    tabId: 7,
    frameId: 0,
    origin: "https://example.test",
    adapter: { id: "test-adapter", version: "1.0.0" },
    snapshotId: SNAPSHOT_ID,
    ...overrides,
  };
}

function verification(
  overrides: Partial<FinalSnapshotVerification> = {},
): FinalSnapshotVerification {
  return {
    submissionId: SUBMISSION_ID,
    sessionId: SESSION_ID,
    tabId: 7,
    frameId: 0,
    adapterId: "test-adapter",
    adapterVersion: "1.0.0",
    snapshotId: SNAPSHOT_ID,
    ...overrides,
  };
}

function originalApproval(
  overrides: Partial<OriginalApprovalConfirmation> = {},
): OriginalApprovalConfirmation {
  return {
    submissionId: SUBMISSION_ID,
    snapshotId: SNAPSHOT_ID,
    confirmed: true,
    ...overrides,
  };
}

interface SummaryInput {
  readonly snapshotId?: string;
  readonly coverage?: ScanCoverage;
  readonly findings?: Partial<Record<GuardSeverity, number>>;
  readonly candidates?: number;
}

function makeSummary(input: SummaryInput = {}): ScanSummary {
  const findingsBySeverity: Record<GuardSeverity, number> = {
    info: 0,
    low: 0,
    medium: 0,
    high: 0,
    critical: 0,
  };
  for (const severity of GUARD_SEVERITIES) {
    findingsBySeverity[severity] = input.findings?.[severity] ?? 0;
  }
  const findingCount = GUARD_SEVERITIES.reduce(
    (total, severity) => total + findingsBySeverity[severity],
    0,
  );
  return {
    snapshotId: input.snapshotId ?? SNAPSHOT_ID,
    coverage: input.coverage ?? "complete",
    findingCount,
    candidateCount: input.candidates ?? 0,
    findingsBySeverity,
  };
}

function scanSucceeded(
  summary: ScanSummary = makeSummary(),
  stage: ScanStage = "initial",
  submissionId: string = SUBMISSION_ID,
): ScanSucceededEvent {
  return { type: "scan_succeeded", submissionId, stage, summary };
}

function scanFailed(
  reason: ScanFailureReason = "scanner_error",
  stage: ScanStage = "initial",
  submissionId: string = SUBMISSION_ID,
): ScanFailedEvent {
  return { type: "scan_failed", submissionId, stage, reason };
}

function transformSucceeded(
  transformedSnapshotId: string = TRANSFORMED_SNAPSHOT_ID,
  snapshotId: string = SNAPSHOT_ID,
  submissionId: string = SUBMISSION_ID,
): TransformSucceededEvent {
  return {
    type: "transform_succeeded",
    submissionId,
    snapshotId,
    transformedSnapshotId,
  };
}

function transformFailed(
  reason: ScanFailureReason = "scanner_error",
  snapshotId: string = SNAPSHOT_ID,
  submissionId: string = SUBMISSION_ID,
): TransformFailedEvent {
  return { type: "transform_failed", submissionId, snapshotId, reason };
}

function expectState<T extends GuardPhase>(
  state: GuardState,
  phase: T,
): Extract<GuardState, { phase: T }> {
  assert.equal(state.phase, phase);
  return state as Extract<GuardState, { phase: T }>;
}

function expectAccepted(result: TransitionResult): void {
  assert.equal(result.rejection, undefined);
}

function expectRejected(
  result: TransitionResult,
  rejection?: TransitionRejection,
): void {
  assert.notEqual(result.rejection, undefined);
  if (rejection !== undefined) {
    assert.equal(result.rejection, rejection);
  }
  assert.deepEqual(result.commands, []);
}

function begin(
  preference: TransformationPreference = DEFAULT_TRANSFORMATION_PREFERENCE,
): TransitionResult {
  return transition(initialGuardState({ preference }), {
    type: "begin",
    transaction: transaction(),
  });
}

function scanningState(
  preference: TransformationPreference = DEFAULT_TRANSFORMATION_PREFERENCE,
): GuardState {
  const result = begin(preference);
  expectAccepted(result);
  return result.state;
}

function scan(
  summary: ScanSummary,
  preference: TransformationPreference = DEFAULT_TRANSFORMATION_PREFERENCE,
): TransitionResult {
  return transition(scanningState(preference), scanSucceeded(summary));
}

function reviewState(
  summary: ScanSummary = makeSummary({ findings: { high: 1 } }),
  preference: TransformationPreference = DEFAULT_TRANSFORMATION_PREFERENCE,
): GuardState {
  const result = scan(summary, preference);
  expectAccepted(result);
  return expectState(result.state, "awaiting_review");
}

function transformingState(): GuardState {
  const result = transition(reviewState(), {
    type: "apply_transformation",
    submissionId: SUBMISSION_ID,
  });
  expectAccepted(result);
  return expectState(result.state, "transforming");
}

function revalidatingState(): GuardState {
  const result = transition(transformingState(), transformSucceeded());
  expectAccepted(result);
  return expectState(result.state, "revalidating");
}

function authorizedCleanState(): GuardState {
  const result = scan(makeSummary());
  expectAccepted(result);
  return expectState(result.state, "authorized");
}

function submittingState(): GuardState {
  const result = transition(authorizedCleanState(), {
    type: "submit_requested",
    verification: verification(),
  });
  expectAccepted(result);
  return expectState(result.state, "submitting");
}

describe("scan summary validation and projection", () => {
  test("summarizeScan counts typed findings and candidates without source", () => {
    const result: ScanResult = {
      sourceBytes: 42,
      findings: [
        {
          ruleId: "rule-a",
          span: { start: 0, end: 1 },
          line: 1,
          column: 1,
          severity: "high",
          confidence: "high",
          remediation: null,
        },
        {
          ruleId: "rule-b",
          span: { start: 2, end: 3 },
          line: 1,
          column: 3,
          severity: "critical",
          confidence: "medium",
          remediation: null,
        },
      ],
      candidates: [
        {
          kind: "unknown",
          evidence: "unknown",
          span: { start: 4, end: 5 },
          line: 1,
          column: 5,
          explanation: { kind: "unknown" },
        },
      ],
      findingCount: 2,
      candidateCount: 1,
      needsReview: true,
      hasCritical: true,
    };

    const summary = summarizeScan(SNAPSHOT_ID, "complete", result);
    assert.equal(summary.findingCount, 2);
    assert.equal(summary.candidateCount, 1);
    assert.equal(summary.findingsBySeverity.high, 1);
    assert.equal(summary.findingsBySeverity.critical, 1);
    assert.deepEqual(Object.keys(summary).sort(), [
      "candidateCount",
      "coverage",
      "findingCount",
      "findingsBySeverity",
      "snapshotId",
    ]);
  });

  test("parseScanSummary accepts a valid summary", () => {
    const summary = makeSummary({ findings: { low: 1, high: 2 }, candidates: 3 });
    assert.deepEqual(parseScanSummary(summary), summary);
  });

  const malformedSummaries: ReadonlyArray<{
    readonly name: string;
    readonly value: unknown;
  }> = [
    { name: "null", value: null },
    { name: "array", value: [] },
    { name: "missing snapshot", value: { ...makeSummary(), snapshotId: "" } },
    { name: "unknown coverage", value: { ...makeSummary(), coverage: "partial" } },
    { name: "negative findings", value: { ...makeSummary(), findingCount: -1 } },
    {
      name: "fractional candidates",
      value: { ...makeSummary(), candidateCount: 1.5 },
    },
    {
      name: "severity total mismatch",
      value: { ...makeSummary({ findings: { high: 1 } }), findingCount: 2 },
    },
    {
      name: "missing severity count",
      value: {
        ...makeSummary(),
        findingsBySeverity: { info: 0, low: 0, medium: 0, high: 0 },
      },
    },
    {
      name: "negative severity count",
      value: {
        ...makeSummary(),
        findingsBySeverity: {
          info: 0,
          low: 0,
          medium: 0,
          high: -1,
          critical: 0,
        },
        findingCount: -1,
      },
    },
  ];

  for (const { name, value } of malformedSummaries) {
    test(`parseScanSummary rejects ${name}`, () => {
      assert.equal(parseScanSummary(value), undefined);
    });
  }

  test("isValidTransactionIdentity accepts a complete identity", () => {
    assert.equal(isValidTransactionIdentity(transaction()), true);
  });

  const malformedIdentities: ReadonlyArray<{
    readonly name: string;
    readonly value: unknown;
  }> = [
    { name: "null", value: null },
    { name: "empty submission", value: { ...transaction(), submissionId: "" } },
    { name: "negative tab", value: { ...transaction(), tabId: -1 } },
    { name: "fractional frame", value: { ...transaction(), frameId: 0.5 } },
    { name: "empty origin", value: { ...transaction(), origin: "" } },
    {
      name: "missing adapter",
      value: { ...transaction(), adapter: undefined },
    },
    {
      name: "empty adapter version",
      value: { ...transaction(), adapter: { id: "a", version: "" } },
    },
    { name: "empty snapshot", value: { ...transaction(), snapshotId: "" } },
  ];

  for (const { name, value } of malformedIdentities) {
    test(`isValidTransactionIdentity rejects ${name}`, () => {
      assert.equal(isValidTransactionIdentity(value), false);
    });
  }
});

describe("policy decision matrix", () => {
  test("clean complete coverage is eligible for normal submission", () => {
    const decision = evaluatePolicy(makeSummary());
    assert.equal(decision.kind, "clean");
    assert.equal(decision.snapshotId, SNAPSHOT_ID);
  });

  for (const severity of GUARD_SEVERITIES) {
    test(`any ${severity} finding requires review`, () => {
      const decision = evaluatePolicy(makeSummary({ findings: { [severity]: 1 } }));
      if (decision.kind !== "review") throw new Error("expected review");
      assert.equal(decision.reason, "findings");
      assert.equal(decision.canTransform, true);
      assert.equal(decision.canApproveOriginal, true);
      assert.equal(
        decision.primaryTransformation,
        DEFAULT_TRANSFORMATION_PREFERENCE,
      );
    });
  }

  test("a candidate-only result requires review and disables transformation", () => {
    const decision = evaluatePolicy(makeSummary({ candidates: 1 }));
    if (decision.kind !== "review") throw new Error("expected review");
    assert.equal(decision.reason, "candidates");
    assert.equal(decision.canTransform, false);
    assert.equal(decision.canApproveOriginal, true);
  });

  test("mixed findings and candidates require review and disable transformation", () => {
    const decision = evaluatePolicy(
      makeSummary({ findings: { high: 1 }, candidates: 2 }),
    );
    if (decision.kind !== "review") throw new Error("expected review");
    assert.equal(decision.reason, "findings_and_candidates");
    assert.equal(decision.canTransform, false);
    assert.equal(decision.canApproveOriginal, true);
  });

  test("unsupported coverage cannot be clean, transformed, or sent as original", () => {
    const decision = evaluatePolicy(makeSummary({ coverage: "unsupported" }));
    if (decision.kind !== "review") throw new Error("expected review");
    assert.equal(decision.reason, "unsupported_coverage");
    assert.equal(decision.canTransform, false);
    assert.equal(decision.canApproveOriginal, false);
  });

  test("unsupported coverage stays unsupported even with findings or candidates", () => {
    const cases: ScanSummary[] = [
      makeSummary({ coverage: "unsupported", findings: { critical: 1 } }),
      makeSummary({ coverage: "unsupported", candidates: 1 }),
      makeSummary({
        coverage: "unsupported",
        findings: { high: 1 },
        candidates: 1,
      }),
    ];
    for (const summary of cases) {
      const decision = evaluatePolicy(summary);
      if (decision.kind !== "review") throw new Error("expected review");
      assert.equal(decision.reason, "unsupported_coverage");
      assert.equal(decision.canTransform, false);
      assert.equal(decision.canApproveOriginal, false);
    }
  });

  test("template preference changes the primary transformation only", () => {
    const decision = evaluatePolicy(
      makeSummary({ findings: { low: 1 } }),
      "template",
    );
    if (decision.kind !== "review") throw new Error("expected review");
    assert.equal(decision.primaryTransformation, "template");
    assert.equal(decision.canTransform, true);
  });

  test("redact remains the default preference", () => {
    const decision = evaluatePolicy(makeSummary({ findings: { low: 1 } }));
    if (decision.kind !== "review") throw new Error("expected review");
    assert.equal(decision.primaryTransformation, "redact");
  });

  test("the v1 decision matrix is table-driven and exhaustive", () => {
    interface ExpectedDecision {
      readonly kind: "clean" | "review";
      readonly reason?: ReviewReason;
      readonly canTransform?: boolean;
      readonly canApproveOriginal?: boolean;
      readonly primaryTransformation?: TransformationPreference;
    }

    const cases: ReadonlyArray<{
      readonly name: string;
      readonly summary: ScanSummary;
      readonly preference?: TransformationPreference;
      readonly expected: ExpectedDecision;
    }> = [
      {
        name: "clean",
        summary: makeSummary(),
        expected: { kind: "clean" },
      },
      ...GUARD_SEVERITIES.map((severity) => ({
        name: `finding ${severity}`,
        summary: makeSummary({ findings: { [severity]: 1 } }),
        expected: {
          kind: "review" as const,
          reason: "findings" as const,
          canTransform: true,
          canApproveOriginal: true,
          primaryTransformation: "redact" as const,
        },
      })),
      {
        name: "candidate only",
        summary: makeSummary({ candidates: 1 }),
        expected: {
          kind: "review",
          reason: "candidates",
          canTransform: false,
          canApproveOriginal: true,
          primaryTransformation: "redact",
        },
      },
      {
        name: "mixed findings and candidates",
        summary: makeSummary({ findings: { high: 1 }, candidates: 1 }),
        expected: {
          kind: "review",
          reason: "findings_and_candidates",
          canTransform: false,
          canApproveOriginal: true,
          primaryTransformation: "redact",
        },
      },
      {
        name: "unsupported coverage",
        summary: makeSummary({ coverage: "unsupported" }),
        expected: {
          kind: "review",
          reason: "unsupported_coverage",
          canTransform: false,
          canApproveOriginal: false,
          primaryTransformation: "redact",
        },
      },
      {
        name: "unsupported coverage with a finding",
        summary: makeSummary({
          coverage: "unsupported",
          findings: { critical: 1 },
        }),
        expected: {
          kind: "review",
          reason: "unsupported_coverage",
          canTransform: false,
          canApproveOriginal: false,
          primaryTransformation: "redact",
        },
      },
      {
        name: "template preference",
        summary: makeSummary({ findings: { medium: 1 } }),
        preference: "template",
        expected: {
          kind: "review",
          reason: "findings",
          canTransform: true,
          canApproveOriginal: true,
          primaryTransformation: "template",
        },
      },
    ];

    for (const { name, summary, preference, expected } of cases) {
      const decision = evaluatePolicy(summary, preference);
      assert.equal(decision.kind, expected.kind, name);
      if (decision.kind === "review") {
        assert.equal(decision.reason, expected.reason, name);
        assert.equal(decision.canTransform, expected.canTransform, name);
        assert.equal(
          decision.canApproveOriginal,
          expected.canApproveOriginal,
          name,
        );
        assert.equal(
          decision.primaryTransformation,
          expected.primaryTransformation ??
            preference ??
            DEFAULT_TRANSFORMATION_PREFERENCE,
          name,
        );
      }
    }
  });
});

describe("submission state machine", () => {
  test("clean scan authorizes only its snapshot and requires final verification", () => {
    const began = begin();
    expectAccepted(began);
    assert.equal(began.state.phase, "scanning");
    assert.deepEqual(began.commands, [
      {
        type: "scan",
        submissionId: SUBMISSION_ID,
        snapshotId: SNAPSHOT_ID,
        stage: "initial",
      },
    ]);

    const scanned = transition(began.state, scanSucceeded());
    expectAccepted(scanned);
    const authorized = expectState(scanned.state, "authorized");
    assert.equal(authorized.authorization.kind, "clean");
    assert.equal(authorized.authorization.snapshotId, SNAPSHOT_ID);
    assert.equal(authorized.authorization.consumed, false);
    assert.deepEqual(scanned.commands, []);

    const submitted = transition(authorized, {
      type: "submit_requested",
      verification: verification(),
    });
    expectAccepted(submitted);
    const submitting = expectState(submitted.state, "submitting");
    assert.equal(submitting.authorization.consumed, true);
    assert.deepEqual(submitted.commands, [
      {
        type: "submit",
        submissionId: SUBMISSION_ID,
        snapshotId: SNAPSHOT_ID,
        authorizationId: `${SUBMISSION_ID}:${SNAPSHOT_ID}:clean`,
        authorizationKind: "clean",
      },
    ]);

    const completed = transition(submitting, {
      type: "submit_succeeded",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(completed);
    const done = expectState(completed.state, "completed");
    assert.equal(done.confirmation, "adapter_submitted");
  });

  test("a confirmed snapshot mismatch irreversibly invalidates authorization", () => {
    const state = authorizedCleanState();
    const result = transition(state, {
      type: "submit_requested",
      verification: verification({ snapshotId: OTHER_SNAPSHOT_ID }),
    });
    expectAccepted(result);
    const blocked = expectState(result.state, "blocked");
    assert.equal(blocked.reason, "snapshot_mismatch");
    assert.deepEqual(result.commands, []);
    assert.equal("authorization" in blocked, false);

    // The invalidation is terminal: neither a late valid verification nor a
    // source change back to the previous value can restore the authorization.
    expectRejected(
      transition(blocked, {
        type: "submit_requested",
        verification: verification(),
      }),
      "terminal_state",
    );
    expectRejected(
      transition(blocked, {
        type: "source_changed",
        submissionId: SUBMISSION_ID,
        snapshotId: SNAPSHOT_ID,
      }),
      "terminal_state",
    );
    expectRejected(
      transition(blocked, {
        type: "begin",
        transaction: transaction({ submissionId: "submission-2" }),
      }),
      "terminal_state",
    );
  });

  test("a snapshot mismatch followed by restored content still requires a fresh scan", () => {
    const mismatch = transition(authorizedCleanState(), {
      type: "submit_requested",
      verification: verification({ snapshotId: OTHER_SNAPSHOT_ID }),
    });
    expectAccepted(mismatch);
    expectState(mismatch.state, "blocked");

    // A new submission starts from a fresh machine, and the first command is a
    // fresh scan. No authorization survives the mismatch.
    const restarted = transition(initialGuardState(), {
      type: "begin",
      transaction: transaction({ submissionId: "submission-2" }),
    });
    expectAccepted(restarted);
    expectState(restarted.state, "scanning");
    assert.deepEqual(restarted.commands, [
      {
        type: "scan",
        submissionId: "submission-2",
        snapshotId: SNAPSHOT_ID,
        stage: "initial",
      },
    ]);
  });

  test("stale or unrelated verification events are rejected without consuming authorization", () => {
    const unrelated: ReadonlyArray<Partial<FinalSnapshotVerification>> = [
      { sessionId: "other-session" },
      { tabId: 8 },
      { frameId: 1 },
      { adapterId: "other-adapter" },
      { adapterVersion: "9.9.9" },
    ];
    for (const mismatch of unrelated) {
      const result = transition(authorizedCleanState(), {
        type: "submit_requested",
        verification: verification(mismatch),
      });
      expectRejected(result, "verification_identity_mismatch");
      const authorized = expectState(result.state, "authorized");
      assert.equal(authorized.authorization.consumed, false);
    }

    const otherSubmission = transition(authorizedCleanState(), {
      type: "submit_requested",
      verification: verification({ submissionId: "other-submission" }),
    });
    expectRejected(otherSubmission, "stale_transaction");
    const stillAuthorized = expectState(otherSubmission.state, "authorized");
    assert.equal(stillAuthorized.authorization.consumed, false);

    // After stale events the real transaction can still be authorized once.
    const valid = transition(stillAuthorized, {
      type: "submit_requested",
      verification: verification(),
    });
    expectAccepted(valid);
    assert.equal(valid.commands.length, 1);
  });

  test("malformed verification never emits a submit command", () => {
    const malformed: ReadonlyArray<unknown> = [
      null,
      undefined,
      {},
      { ...verification(), tabId: -1 },
      { ...verification(), snapshotId: "" },
      "verification",
    ];
    for (const value of malformed) {
      const result = transition(authorizedCleanState(), {
        type: "submit_requested",
        verification: value as FinalSnapshotVerification,
      });
      expectRejected(result, "snapshot_verification_failed");
      expectState(result.state, "authorized");
      assert.deepEqual(result.commands, []);
    }
  });

  for (const severity of GUARD_SEVERITIES) {
    test(`a ${severity} finding requires review and offers the configured transform`, () => {
      const result = scan(makeSummary({ findings: { [severity]: 1 } }));
      expectAccepted(result);
      const state = expectState(result.state, "awaiting_review");
      assert.equal(state.decision.reason, "findings");
      assert.equal(state.decision.canTransform, true);

      const applied = transition(state, {
        type: "apply_transformation",
        submissionId: SUBMISSION_ID,
      });
      expectAccepted(applied);
      expectState(applied.state, "transforming");
      assert.deepEqual(applied.commands, [
        {
          type: "transform",
          submissionId: SUBMISSION_ID,
          snapshotId: SNAPSHOT_ID,
          preference: "redact",
        },
      ]);
    });
  }

  test("unsupported coverage offers review and edit but never authorization", () => {
    const summary = makeSummary({ coverage: "unsupported" });
    const awaiting = expectState(reviewState(summary), "awaiting_review");
    assert.equal(awaiting.decision.reason, "unsupported_coverage");
    assert.equal(awaiting.decision.canTransform, false);
    assert.equal(awaiting.decision.canApproveOriginal, false);

    const transformed = transition(awaiting, {
      type: "apply_transformation",
      submissionId: SUBMISSION_ID,
    });
    expectRejected(transformed, "transformation_unavailable");
    assert.deepEqual(transformed.commands, []);

    const approved = transition(awaiting, {
      type: "approve_original",
      confirmation: originalApproval(),
    });
    expectRejected(approved, "original_send_unavailable");
    assert.deepEqual(approved.commands, []);

    const submit = transition(awaiting, {
      type: "submit_requested",
      verification: verification(),
    });
    expectRejected(submit, "invalid_transition");
    assert.deepEqual(submit.commands, []);

    // Review/edit and cancellation remain available.
    const edited = transition(awaiting, {
      type: "review_edit",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(edited);
    assert.equal(expectState(edited.state, "cancelled").reason, "user_review_edit");

    const cancelled = transition(awaiting, {
      type: "cancel",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(cancelled);
    assert.equal(expectState(cancelled.state, "cancelled").reason, "user_cancelled");
  });

  test("unsupported coverage is distinct from a complete scan with findings", () => {
    const unsupported = expectState(
      reviewState(makeSummary({ coverage: "unsupported" })),
      "awaiting_review",
    );
    const findings = expectState(
      reviewState(makeSummary({ findings: { high: 1 } })),
      "awaiting_review",
    );

    assert.equal(unsupported.decision.reason, "unsupported_coverage");
    assert.equal(findings.decision.reason, "findings");
    assert.equal(unsupported.decision.canApproveOriginal, false);
    assert.equal(findings.decision.canApproveOriginal, true);

    // Explicit one-shot original approval remains available for complete scans.
    const approved = transition(findings, {
      type: "approve_original",
      confirmation: originalApproval(),
    });
    expectAccepted(approved);
    assert.equal(expectState(approved.state, "authorized").authorization.kind, "original");
  });

  test("candidate-only results are review-only and cannot be transformed", () => {
    const state = reviewState(makeSummary({ candidates: 1 }));
    const awaiting = expectState(state, "awaiting_review");
    assert.equal(awaiting.decision.reason, "candidates");
    assert.equal(awaiting.decision.canTransform, false);

    const applied = transition(awaiting, {
      type: "apply_transformation",
      submissionId: SUBMISSION_ID,
    });
    expectRejected(applied, "transformation_unavailable");
    assert.equal(applied.state, awaiting);
  });

  test("mixed findings and candidates disable transformation", () => {
    const state = reviewState(
      makeSummary({ findings: { critical: 1 }, candidates: 1 }),
    );
    const awaiting = expectState(state, "awaiting_review");
    assert.equal(awaiting.decision.reason, "findings_and_candidates");
    assert.equal(awaiting.decision.canTransform, false);

    expectRejected(
      transition(awaiting, {
        type: "apply_transformation",
        submissionId: SUBMISSION_ID,
      }),
      "transformation_unavailable",
    );
  });

  test("template preference determines the offered transformation", () => {
    const state = reviewState(
      makeSummary({ findings: { medium: 1 } }),
      "template",
    );
    const awaiting = expectState(state, "awaiting_review");
    assert.equal(awaiting.decision.primaryTransformation, "template");

    const applied = transition(awaiting, {
      type: "apply_transformation",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(applied);
    assert.deepEqual(applied.commands, [
      {
        type: "transform",
        submissionId: SUBMISSION_ID,
        snapshotId: SNAPSHOT_ID,
        preference: "template",
      },
    ]);
  });

  test("explicit original approval is one-shot and bound to the snapshot", () => {
    const state = reviewState();
    const approved = transition(state, {
      type: "approve_original",
      confirmation: originalApproval(),
    });
    expectAccepted(approved);
    const authorized = expectState(approved.state, "authorized");
    assert.equal(authorized.authorization.kind, "original");
    assert.equal(authorized.authorization.snapshotId, SNAPSHOT_ID);

    const submitted = transition(authorized, {
      type: "submit_requested",
      verification: verification(),
    });
    expectAccepted(submitted);
    const submitting = expectState(submitted.state, "submitting");
    assert.equal(submitting.authorization.consumed, true);

    const repeated = transition(submitting, {
      type: "submit_requested",
      verification: verification(),
    });
    expectRejected(repeated, "authorization_consumed");
    expectState(repeated.state, "submitting");
  });

  test("original approval rejects a mismatched submission or snapshot", () => {
    const state = reviewState();

    expectRejected(
      transition(state, {
        type: "approve_original",
        confirmation: originalApproval({ submissionId: "other-submission" }),
      }),
      "stale_transaction",
    );

    expectRejected(
      transition(state, {
        type: "approve_original",
        confirmation: originalApproval({ snapshotId: OTHER_SNAPSHOT_ID }),
      }),
      "consent_mismatch",
    );

    const unconfirmed = {
      submissionId: SUBMISSION_ID,
      snapshotId: SNAPSHOT_ID,
      confirmed: false,
    } as unknown as OriginalApprovalConfirmation;
    expectRejected(
      transition(state, { type: "approve_original", confirmation: unconfirmed }),
      "confirmation_required",
    );

    expectRejected(
      transition(state, {
        type: "approve_original",
        confirmation: null as unknown as OriginalApprovalConfirmation,
      }),
      "confirmation_required",
    );
  });

  test("review and edit cancels the transaction without authorization", () => {
    const result = transition(reviewState(), {
      type: "review_edit",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(result);
    const cancelled = expectState(result.state, "cancelled");
    assert.equal(cancelled.reason, "user_review_edit");

    expectRejected(
      transition(cancelled, {
        type: "approve_original",
        confirmation: originalApproval(),
      }),
      "terminal_state",
    );
  });

  test("explicit cancellation is terminal and cannot be revived", () => {
    const result = transition(scanningState(), {
      type: "cancel",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(result);
    const cancelled = expectState(result.state, "cancelled");
    assert.equal(cancelled.reason, "user_cancelled");

    expectRejected(
      transition(cancelled, scanSucceeded()),
      "terminal_state",
    );
  });

  test("scanner failure blocks and offers no original-send bypass", () => {
    const result = transition(scanningState(), scanFailed());
    expectAccepted(result);
    const blocked = expectState(result.state, "blocked");
    assert.equal(blocked.reason, "scan_failed");

    expectRejected(
      transition(blocked, {
        type: "approve_original",
        confirmation: originalApproval(),
      }),
      "terminal_state",
    );
  });

  test("malformed scan responses block instead of authorizing", () => {
    const state = scanningState();
    const malformed = {
      ...makeSummary(),
      findingCount: -1,
    } as ScanSummary;
    const result = transition(state, scanSucceeded(malformed));
    expectAccepted(result);
    const blocked = expectState(result.state, "blocked");
    assert.equal(blocked.reason, "malformed_scan_response");
  });

  test("a null scan summary blocks instead of throwing", () => {
    const result = transition(
      scanningState(),
      scanSucceeded(null as unknown as ScanSummary),
    );
    expectAccepted(result);
    const blocked = expectState(result.state, "blocked");
    assert.equal(blocked.reason, "malformed_scan_response");
  });

  test("timeout and disconnect prevent authorization", () => {
    const timeout = transition(scanningState(), {
      type: "timeout",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(timeout);
    assert.equal(expectState(timeout.state, "blocked").reason, "timeout");

    const disconnect = transition(reviewState(), {
      type: "scanner_disconnected",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(disconnect);
    assert.equal(
      expectState(disconnect.state, "blocked").reason,
      "scanner_disconnected",
    );

    const authorizedTimeout = transition(authorizedCleanState(), {
      type: "timeout",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(authorizedTimeout);
    assert.equal(expectState(authorizedTimeout.state, "blocked").reason, "timeout");
  });

  test("transformation failure blocks and never falls back to original", () => {
    const result = transition(transformingState(), transformFailed());
    expectAccepted(result);
    const blocked = expectState(result.state, "blocked");
    assert.equal(blocked.reason, "transformation_failed");

    expectRejected(
      transition(blocked, {
        type: "approve_original",
        confirmation: originalApproval(),
      }),
      "terminal_state",
    );
  });

  test("malformed transform responses block", () => {
    const result = transition(
      transformingState(),
      transformFailed("malformed_response"),
    );
    expectAccepted(result);
    assert.equal(
      expectState(result.state, "blocked").reason,
      "malformed_transform_response",
    );
  });

  test("transformed output requires a fresh scan before authorization", () => {
    const transformed = transition(transformingState(), transformSucceeded());
    expectAccepted(transformed);
    const revalidating = expectState(transformed.state, "revalidating");
    assert.equal(revalidating.transformedSnapshotId, TRANSFORMED_SNAPSHOT_ID);
    assert.deepEqual(transformed.commands, [
      {
        type: "scan",
        submissionId: SUBMISSION_ID,
        snapshotId: TRANSFORMED_SNAPSHOT_ID,
        stage: "transformed",
      },
    ]);

    const rescanned = transition(
      revalidating,
      scanSucceeded(
        makeSummary({ snapshotId: TRANSFORMED_SNAPSHOT_ID }),
        "transformed",
      ),
    );
    expectAccepted(rescanned);
    const authorized = expectState(rescanned.state, "authorized");
    assert.equal(authorized.authorization.kind, "transformed");
    assert.equal(authorized.authorization.snapshotId, TRANSFORMED_SNAPSHOT_ID);

    const wrongSnapshot = transition(authorized, {
      type: "submit_requested",
      verification: verification({ snapshotId: SNAPSHOT_ID }),
    });
    expectAccepted(wrongSnapshot);
    assert.equal(
      expectState(wrongSnapshot.state, "blocked").reason,
      "snapshot_mismatch",
    );
    assert.deepEqual(wrongSnapshot.commands, []);

    const submitted = transition(
      transition(
        revalidating,
        scanSucceeded(
          makeSummary({ snapshotId: TRANSFORMED_SNAPSHOT_ID }),
          "transformed",
        ),
      ).state,
      {
        type: "submit_requested",
        verification: verification({ snapshotId: TRANSFORMED_SNAPSHOT_ID }),
      },
    );
    expectAccepted(submitted);
    expectState(submitted.state, "submitting");
  });

  const residualCases: ReadonlyArray<{
    readonly name: string;
    readonly summary: ScanSummary;
    readonly reason:
      | "residual_findings"
      | "residual_candidates"
      | "residual_findings_and_candidates"
      | "unsupported_coverage";
  }> = [
    {
      name: "residual findings",
      summary: makeSummary({
        snapshotId: TRANSFORMED_SNAPSHOT_ID,
        findings: { info: 1 },
      }),
      reason: "residual_findings",
    },
    {
      name: "residual candidates",
      summary: makeSummary({
        snapshotId: TRANSFORMED_SNAPSHOT_ID,
        candidates: 1,
      }),
      reason: "residual_candidates",
    },
    {
      name: "residual findings and candidates",
      summary: makeSummary({
        snapshotId: TRANSFORMED_SNAPSHOT_ID,
        findings: { high: 1 },
        candidates: 1,
      }),
      reason: "residual_findings_and_candidates",
    },
    {
      name: "unsupported coverage",
      summary: makeSummary({
        snapshotId: TRANSFORMED_SNAPSHOT_ID,
        coverage: "unsupported",
      }),
      reason: "unsupported_coverage",
    },
  ];

  for (const { name, summary, reason } of residualCases) {
    test(`${name} blocks transformed submission`, () => {
      const result = transition(
        revalidatingState(),
        scanSucceeded(summary, "transformed"),
      );
      expectAccepted(result);
      const blocked = expectState(result.state, "blocked");
      assert.equal(blocked.reason, reason);
    });
  }

  test("a stale scan snapshot is rejected without changing state", () => {
    const state = scanningState();
    const result = transition(
      state,
      scanSucceeded(makeSummary({ snapshotId: OTHER_SNAPSHOT_ID })),
    );
    expectRejected(result, "stale_snapshot");
    assert.equal(result.state, state);
  });

  test("scan results for the wrong stage are rejected", () => {
    const scanning = scanningState();
    expectRejected(
      transition(scanning, scanSucceeded(makeSummary(), "transformed")),
      "stale_stage",
    );

    const revalidating = revalidatingState();
    expectRejected(
      transition(revalidating, scanSucceeded(makeSummary(), "initial")),
      "stale_stage",
    );

    const failed = transition(
      scanningState(),
      scanFailed("scanner_error", "transformed"),
    );
    expectRejected(failed, "stale_stage");
    expectState(failed.state, "scanning");
  });

  test("a transformed snapshot must be fresh and correlate to the source", () => {
    const sameSnapshot = transition(
      transformingState(),
      transformSucceeded(SNAPSHOT_ID),
    );
    expectRejected(sameSnapshot, "stale_snapshot");
    expectState(sameSnapshot.state, "transforming");

    const otherSource = transition(
      transformingState(),
      transformSucceeded(TRANSFORMED_SNAPSHOT_ID, OTHER_SNAPSHOT_ID),
    );
    expectRejected(otherSource, "stale_snapshot");
    expectState(otherSource.state, "transforming");

    const otherFailure = transition(
      transformingState(),
      transformFailed("scanner_error", OTHER_SNAPSHOT_ID),
    );
    expectRejected(otherFailure, "stale_snapshot");
    expectState(otherFailure.state, "transforming");
  });

  test("source changes invalidate an existing authorization", () => {
    const authorized = authorizedCleanState();
    const result = transition(authorized, {
      type: "source_changed",
      submissionId: SUBMISSION_ID,
      snapshotId: OTHER_SNAPSHOT_ID,
    });
    expectAccepted(result);
    const cancelled = expectState(result.state, "cancelled");
    assert.equal(cancelled.reason, "source_changed");

    expectRejected(
      transition(cancelled, {
        type: "submit_requested",
        verification: verification(),
      }),
      "terminal_state",
    );
  });

  test("navigation, document replacement, and adapter invalidation cancel", () => {
    const cases: ReadonlyArray<{
      readonly state: GuardState;
      readonly event: GuardEvent;
      readonly reason:
        | "navigated"
        | "document_replaced"
        | "adapter_invalidated";
    }> = [
      {
        state: authorizedCleanState(),
        event: { type: "navigated", submissionId: SUBMISSION_ID },
        reason: "navigated",
      },
      {
        state: reviewState(),
        event: { type: "document_replaced", submissionId: SUBMISSION_ID },
        reason: "document_replaced",
      },
      {
        state: transformingState(),
        event: { type: "adapter_invalidated", submissionId: SUBMISSION_ID },
        reason: "adapter_invalidated",
      },
      {
        state: revalidatingState(),
        event: { type: "navigated", submissionId: SUBMISSION_ID },
        reason: "navigated",
      },
    ];

    for (const { state, event, reason } of cases) {
      const result = transition(state, event);
      expectAccepted(result);
      const cancelled = expectState(result.state, "cancelled");
      assert.equal(cancelled.reason, reason);
    }
  });

  test("late scan and transform results cannot advance a later state", () => {
    const awaiting = reviewState();
    expectRejected(
      transition(awaiting, scanSucceeded()),
      "invalid_transition",
    );

    const authorized = authorizedCleanState();
    expectRejected(
      transition(authorized, transformSucceeded()),
      "invalid_transition",
    );

    const completed = transition(submittingState(), {
      type: "submit_succeeded",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(completed);
    expectRejected(
      transition(completed.state, scanSucceeded()),
      "terminal_state",
    );
  });

  test("events for another transaction are rejected", () => {
    const state = scanningState();
    const result = transition(
      state,
      scanSucceeded(makeSummary(), "initial", "other-submission"),
    );
    expectRejected(result, "stale_transaction");
    assert.equal(result.state, state);

    const authorized = authorizedCleanState();
    const submit = transition(authorized, {
      type: "submit_requested",
      verification: verification({ submissionId: "other-submission" }),
    });
    expectRejected(submit, "stale_transaction");
    assert.deepEqual(submit.commands, []);
    expectState(submit.state, "authorized");
  });

  test("repeated submit attempts cannot produce duplicate sends", () => {
    const first = transition(authorizedCleanState(), {
      type: "submit_requested",
      verification: verification(),
    });
    expectAccepted(first);
    assert.equal(first.commands.length, 1);

    const second = transition(first.state, {
      type: "submit_requested",
      verification: verification(),
    });
    expectRejected(second, "authorization_consumed");
    assert.deepEqual(second.commands, []);
  });

  test("lifecycle invalidation after submit leaves the outcome unknown", () => {
    const cases: ReadonlyArray<{
      readonly event: GuardEvent;
      readonly reason: SubmissionOutcomeUnknownReason;
    }> = [
      { event: { type: "cancel", submissionId: SUBMISSION_ID }, reason: "cancelled" },
      { event: { type: "timeout", submissionId: SUBMISSION_ID }, reason: "timeout" },
      {
        event: { type: "scanner_disconnected", submissionId: SUBMISSION_ID },
        reason: "scanner_disconnected",
      },
      {
        event: {
          type: "source_changed",
          submissionId: SUBMISSION_ID,
          snapshotId: OTHER_SNAPSHOT_ID,
        },
        reason: "source_changed",
      },
      { event: { type: "navigated", submissionId: SUBMISSION_ID }, reason: "navigated" },
      {
        event: { type: "document_replaced", submissionId: SUBMISSION_ID },
        reason: "document_replaced",
      },
      {
        event: { type: "adapter_invalidated", submissionId: SUBMISSION_ID },
        reason: "adapter_invalidated",
      },
    ];

    for (const { event, reason } of cases) {
      const result = transition(submittingState(), event);
      expectAccepted(result);
      const unknown = expectState(result.state, "outcome_unknown");
      assert.equal(unknown.reason, reason);
      assert.equal(unknown.authorization.consumed, true);
      assert.deepEqual(result.commands, []);

      // No late response may revive the transaction or emit a second submit.
      expectRejected(
        transition(unknown, {
          type: "submit_succeeded",
          submissionId: SUBMISSION_ID,
        }),
        "terminal_state",
      );
      expectRejected(
        transition(unknown, {
          type: "submit_requested",
          verification: verification(),
        }),
        "terminal_state",
      );
    }
  });

  test("adapter-level submission failure is distinct from unknown transport outcome", () => {
    const rejected = transition(submittingState(), {
      type: "submit_failed",
      submissionId: SUBMISSION_ID,
      reason: "rejected",
    });
    expectAccepted(rejected);
    assert.equal(expectState(rejected.state, "blocked").reason, "submission_failed");

    const transport = transition(submittingState(), {
      type: "submit_failed",
      submissionId: SUBMISSION_ID,
      reason: "transport_error",
    });
    expectAccepted(transport);
    const unknown = expectState(transport.state, "outcome_unknown");
    assert.equal(unknown.reason, "transport_error");
    assert.equal(unknown.authorization.consumed, true);
    assert.deepEqual(transport.commands, []);
  });

  test("completed records adapter-level confirmation only", () => {
    const completed = transition(submittingState(), {
      type: "submit_succeeded",
      submissionId: SUBMISSION_ID,
    });
    expectAccepted(completed);
    const done = expectState(completed.state, "completed");
    assert.equal(done.confirmation, "adapter_submitted");
    assert.deepEqual(Object.keys(done).sort(), [
      "config",
      "confirmation",
      "phase",
      "transaction",
    ]);
  });

  test("invalid transitions are rejected safely", () => {
    const idle = initialGuardState();
    expectRejected(
      transition(idle, {
        type: "apply_transformation",
        submissionId: SUBMISSION_ID,
      }),
      "invalid_transition",
    );

    expectRejected(transition(idle, scanSucceeded()), "invalid_transition");

    expectRejected(
      transition(scanningState(), {
        type: "approve_original",
        confirmation: originalApproval(),
      }),
      "invalid_transition",
    );

    expectRejected(
      transition(reviewState(), {
        type: "submit_requested",
        verification: verification(),
      }),
      "invalid_transition",
    );

    expectRejected(
      transition(transformingState(), scanSucceeded()),
      "invalid_transition",
    );
  });

  test("an invalid transaction identity is rejected at begin", () => {
    const result = transition(initialGuardState(), {
      type: "begin",
      transaction: { ...transaction(), tabId: -1 },
    });
    expectRejected(result, "invalid_transaction");
    expectState(result.state, "idle");
  });

  test("terminal states cannot be revived", () => {
    const blocked = transition(scanningState(), scanFailed()).state;
    expectRejected(
      transition(blocked, {
        type: "begin",
        transaction: transaction({ submissionId: "submission-2" }),
      }),
      "terminal_state",
    );

    const cancelled = transition(scanningState(), {
      type: "cancel",
      submissionId: SUBMISSION_ID,
    }).state;
    expectRejected(
      transition(cancelled, {
        type: "submit_succeeded",
        submissionId: SUBMISSION_ID,
      }),
      "terminal_state",
    );
  });

  test("the transition function does not mutate its input state", () => {
    const state = authorizedCleanState();
    const snapshot = structuredClone(state);
    transition(state, {
      type: "submit_requested",
      verification: verification(),
    });
    assert.deepEqual(state, snapshot);
  });

  test("state retains only sanitized metadata", () => {
    const state = reviewState();
    assert.deepEqual(Object.keys(state).sort(), [
      "config",
      "decision",
      "phase",
      "summary",
      "transaction",
    ]);
    assert.equal("source" in state, false);
    assert.equal("output" in state, false);
  });
});

describe("state and event coverage", () => {
  function representativeStates(): GuardState[] {
    return [
      initialGuardState(),
      scanningState(),
      reviewState(),
      transformingState(),
      revalidatingState(),
      authorizedCleanState(),
      submittingState(),
      transition(submittingState(), {
        type: "submit_succeeded",
        submissionId: SUBMISSION_ID,
      }).state,
      transition(submittingState(), {
        type: "timeout",
        submissionId: SUBMISSION_ID,
      }).state,
      transition(scanningState(), scanFailed()).state,
      transition(scanningState(), {
        type: "cancel",
        submissionId: SUBMISSION_ID,
      }).state,
    ];
  }

  function representativeEvents(): GuardEvent[] {
    return [
      {
        type: "begin",
        transaction: transaction({
          submissionId: "submission-2",
          snapshotId: "snapshot-9",
        }),
      },
      scanSucceeded(),
      scanSucceeded(makeSummary({ snapshotId: TRANSFORMED_SNAPSHOT_ID }), "transformed"),
      scanFailed(),
      scanFailed("malformed_response"),
      transformSucceeded(),
      transformFailed(),
      { type: "apply_transformation", submissionId: SUBMISSION_ID },
      {
        type: "approve_original",
        confirmation: originalApproval(),
      },
      { type: "review_edit", submissionId: SUBMISSION_ID },
      { type: "cancel", submissionId: SUBMISSION_ID },
      {
        type: "source_changed",
        submissionId: SUBMISSION_ID,
        snapshotId: "snapshot-3",
      },
      { type: "navigated", submissionId: SUBMISSION_ID },
      { type: "document_replaced", submissionId: SUBMISSION_ID },
      { type: "adapter_invalidated", submissionId: SUBMISSION_ID },
      { type: "timeout", submissionId: SUBMISSION_ID },
      { type: "scanner_disconnected", submissionId: SUBMISSION_ID },
      { type: "submit_requested", verification: verification() },
      {
        type: "submit_requested",
        verification: verification({ snapshotId: TRANSFORMED_SNAPSHOT_ID }),
      },
      {
        type: "submit_failed",
        submissionId: SUBMISSION_ID,
        reason: "transport_error",
      },
      { type: "submit_succeeded", submissionId: SUBMISSION_ID },
    ];
  }

  test("all phases are represented", () => {
    const phases: GuardPhase[] = [
      "idle",
      "scanning",
      "awaiting_review",
      "transforming",
      "revalidating",
      "authorized",
      "submitting",
      "completed",
      "outcome_unknown",
      "blocked",
      "cancelled",
    ];
    const seen = new Set(representativeStates().map((state) => state.phase));
    assert.deepEqual([...seen].sort(), phases.slice().sort());
  });

  test("every state and event pair is defined and safe", () => {
    const states = representativeStates();
    const events = representativeEvents();

    for (const state of states) {
      for (const event of events) {
        const result = transition(state, event);
        const label = `${state.phase} + ${event.type}`;
        assert.ok(
          [
            "idle",
            "scanning",
            "awaiting_review",
            "transforming",
            "revalidating",
            "authorized",
            "submitting",
            "completed",
            "outcome_unknown",
            "blocked",
            "cancelled",
          ].includes(result.state.phase),
          label,
        );
        if (result.rejection !== undefined) {
          assert.equal(result.commands.length, 0, label);
        }
        assert.ok(result.commands.length <= 1, label);
      }
    }
  });
});
