/**
 * Pure Guard submission state machine.
 *
 * The machine decides which state is valid next and returns typed commands for
 * a future controller to execute. It never scans, transforms, sends, touches
 * the DOM, reads storage, or writes logs. It retains opaque snapshot references
 * and sanitized counts only.
 */

import { evaluatePolicy, type ReviewDecision } from "./policy.js";
import {
  DEFAULT_TRANSFORMATION_PREFERENCE,
  isValidFinalSnapshotVerification,
  isValidGuardIdentifier,
  isValidOriginalApprovalConfirmation,
  isValidTransactionIdentity,
  parseScanSummary,
  type FinalSnapshotVerification,
  type OriginalApprovalConfirmation,
  type ScanSummary,
  type TransactionIdentity,
  type TransformationPreference,
} from "./types.js";

export interface GuardConfig {
  readonly preference: TransformationPreference;
}

export const DEFAULT_GUARD_CONFIG: GuardConfig = {
  preference: DEFAULT_TRANSFORMATION_PREFERENCE,
};

export type ScanStage = "initial" | "transformed";

export interface IdleState {
  readonly phase: "idle";
  readonly config: GuardConfig;
}

export interface ScanningState {
  readonly phase: "scanning";
  readonly config: GuardConfig;
  readonly transaction: TransactionIdentity;
}

export interface AwaitingReviewState {
  readonly phase: "awaiting_review";
  readonly config: GuardConfig;
  readonly transaction: TransactionIdentity;
  readonly summary: ScanSummary;
  readonly decision: ReviewDecision;
}

export interface TransformingState {
  readonly phase: "transforming";
  readonly config: GuardConfig;
  readonly transaction: TransactionIdentity;
  readonly summary: ScanSummary;
  readonly preference: TransformationPreference;
}

export interface RevalidatingState {
  readonly phase: "revalidating";
  readonly config: GuardConfig;
  readonly transaction: TransactionIdentity;
  readonly transformedSnapshotId: string;
}

export type AuthorizationKind = "clean" | "original" | "transformed";

/**
 * One-shot authorization. `consumed` flips exactly once when the controller
 * presents a valid final snapshot verification.
 */
export interface Authorization {
  readonly kind: AuthorizationKind;
  readonly snapshotId: string;
  readonly consumed: boolean;
}

export interface AuthorizedState {
  readonly phase: "authorized";
  readonly config: GuardConfig;
  readonly transaction: TransactionIdentity;
  readonly authorization: Authorization;
}

export interface SubmittingState {
  readonly phase: "submitting";
  readonly config: GuardConfig;
  readonly transaction: TransactionIdentity;
  readonly authorization: Authorization;
}

/**
 * The only confirmation the pure machine can represent: the adapter reports
 * that it handed the command to the browser or form. Remote delivery
 * confirmation is deliberately not modeled.
 */
export type SubmissionConfirmation = "adapter_submitted";

export interface CompletedState {
  readonly phase: "completed";
  readonly config: GuardConfig;
  readonly transaction: TransactionIdentity;
  /**
   * Adapter-level confirmation only: the controller reports that it handed the
   * submit command to the browser or form. This is not remote delivery
   * confirmation, and it says nothing about the destination server.
   */
  readonly confirmation: SubmissionConfirmation;
}

/**
 * Terminal state for a submission whose command was already emitted but whose
 * external outcome is unknown.
 *
 * Once the submit command leaves the machine, a later cancellation, navigation,
 * timeout, disconnect, or other lifecycle invalidation cannot prove that the
 * external operation did not happen. Authorization stays consumed, no retry is
 * permitted, and no second submit command may be emitted.
 */
export interface OutcomeUnknownState {
  readonly phase: "outcome_unknown";
  readonly config: GuardConfig;
  readonly transaction: TransactionIdentity;
  /** The consumed authorization, retained as evidence that it was spent. */
  readonly authorization: Authorization;
  readonly reason: SubmissionOutcomeUnknownReason;
}

export type SubmissionOutcomeUnknownReason =
  | "cancelled"
  | "navigated"
  | "document_replaced"
  | "adapter_invalidated"
  | "source_changed"
  | "timeout"
  | "scanner_disconnected"
  | "transport_error";

export type BlockReason =
  | "scan_failed"
  | "malformed_scan_response"
  | "transformation_failed"
  | "malformed_transform_response"
  | "timeout"
  | "scanner_disconnected"
  | "residual_findings"
  | "residual_candidates"
  | "residual_findings_and_candidates"
  | "unsupported_coverage"
  | "submission_failed"
  | "snapshot_mismatch";

export interface BlockedState {
  readonly phase: "blocked";
  readonly config: GuardConfig;
  readonly transaction?: TransactionIdentity;
  readonly reason: BlockReason;
}

export type CancelReason =
  | "user_cancelled"
  | "user_review_edit"
  | "source_changed"
  | "navigated"
  | "document_replaced"
  | "adapter_invalidated";

export interface CancelledState {
  readonly phase: "cancelled";
  readonly config: GuardConfig;
  readonly transaction?: TransactionIdentity;
  readonly reason: CancelReason;
}

export type GuardState =
  | IdleState
  | ScanningState
  | AwaitingReviewState
  | TransformingState
  | RevalidatingState
  | AuthorizedState
  | SubmittingState
  | CompletedState
  | OutcomeUnknownState
  | BlockedState
  | CancelledState;

export type GuardPhase = GuardState["phase"];

export interface BeginEvent {
  readonly type: "begin";
  readonly transaction: TransactionIdentity;
}

export interface ScanSucceededEvent {
  readonly type: "scan_succeeded";
  readonly submissionId: string;
  /** The scan stage this result belongs to. Must match the active phase. */
  readonly stage: ScanStage;
  readonly summary: ScanSummary;
}

export type ScanFailureReason = "scanner_error" | "malformed_response";

export interface ScanFailedEvent {
  readonly type: "scan_failed";
  readonly submissionId: string;
  /** The scan stage this failure belongs to. Must match the active phase. */
  readonly stage: ScanStage;
  readonly reason: ScanFailureReason;
}

export interface TransformSucceededEvent {
  readonly type: "transform_succeeded";
  readonly submissionId: string;
  /** The exact source snapshot the transformation was requested for. */
  readonly snapshotId: string;
  readonly transformedSnapshotId: string;
}

export interface TransformFailedEvent {
  readonly type: "transform_failed";
  readonly submissionId: string;
  /** The exact source snapshot the transformation was requested for. */
  readonly snapshotId: string;
  readonly reason: ScanFailureReason;
}

export interface ApplyTransformationEvent {
  readonly type: "apply_transformation";
  readonly submissionId: string;
}

export interface ApproveOriginalEvent {
  readonly type: "approve_original";
  readonly confirmation: OriginalApprovalConfirmation;
}

export interface ReviewEditEvent {
  readonly type: "review_edit";
  readonly submissionId: string;
}

export interface CancelEvent {
  readonly type: "cancel";
  readonly submissionId: string;
}

export interface SourceChangedEvent {
  readonly type: "source_changed";
  readonly submissionId: string;
  readonly snapshotId: string;
}

export interface NavigatedEvent {
  readonly type: "navigated";
  readonly submissionId: string;
}

export interface DocumentReplacedEvent {
  readonly type: "document_replaced";
  readonly submissionId: string;
}

export interface AdapterInvalidatedEvent {
  readonly type: "adapter_invalidated";
  readonly submissionId: string;
}

export interface TimeoutEvent {
  readonly type: "timeout";
  readonly submissionId: string;
}

export interface ScannerDisconnectedEvent {
  readonly type: "scanner_disconnected";
  readonly submissionId: string;
}

export interface SubmitRequestedEvent {
  readonly type: "submit_requested";
  readonly verification: FinalSnapshotVerification;
}

export type SubmitFailureReason = "rejected" | "transport_error";

export interface SubmitFailedEvent {
  readonly type: "submit_failed";
  readonly submissionId: string;
  readonly reason: SubmitFailureReason;
}

export interface SubmitSucceededEvent {
  readonly type: "submit_succeeded";
  readonly submissionId: string;
}

export type GuardEvent =
  | BeginEvent
  | ScanSucceededEvent
  | ScanFailedEvent
  | TransformSucceededEvent
  | TransformFailedEvent
  | ApplyTransformationEvent
  | ApproveOriginalEvent
  | ReviewEditEvent
  | CancelEvent
  | SourceChangedEvent
  | NavigatedEvent
  | DocumentReplacedEvent
  | AdapterInvalidatedEvent
  | TimeoutEvent
  | ScannerDisconnectedEvent
  | SubmitRequestedEvent
  | SubmitFailedEvent
  | SubmitSucceededEvent;

export interface ScanCommand {
  readonly type: "scan";
  readonly submissionId: string;
  readonly snapshotId: string;
  readonly stage: ScanStage;
}

export interface TransformCommand {
  readonly type: "transform";
  readonly submissionId: string;
  readonly snapshotId: string;
  readonly preference: TransformationPreference;
}

export interface SubmitCommand {
  readonly type: "submit";
  readonly submissionId: string;
  readonly snapshotId: string;
  /** Opaque in-process reference; not proof of delivery or protection. */
  readonly authorizationId: string;
  readonly authorizationKind: AuthorizationKind;
}

export type GuardCommand = ScanCommand | TransformCommand | SubmitCommand;

export type TransitionRejection =
  | "invalid_transition"
  | "invalid_transaction"
  | "stale_transaction"
  | "stale_snapshot"
  | "stale_stage"
  | "consent_mismatch"
  | "confirmation_required"
  | "transformation_unavailable"
  | "original_send_unavailable"
  | "snapshot_verification_failed"
  | "verification_identity_mismatch"
  | "authorization_consumed"
  | "terminal_state";

export interface TransitionResult {
  readonly state: GuardState;
  /** Empty whenever `rejection` is set. */
  readonly commands: readonly GuardCommand[];
  readonly rejection?: TransitionRejection;
}

type ActiveState =
  | ScanningState
  | AwaitingReviewState
  | TransformingState
  | RevalidatingState
  | AuthorizedState
  | SubmittingState;

type TerminalState =
  | CompletedState
  | OutcomeUnknownState
  | BlockedState
  | CancelledState;

function isTerminal(state: GuardState): state is TerminalState {
  return (
    state.phase === "completed" ||
    state.phase === "outcome_unknown" ||
    state.phase === "blocked" ||
    state.phase === "cancelled"
  );
}

/** Creates the initial idle state. Redaction is the default preference. */
export function initialGuardState(config: Partial<GuardConfig> = {}): IdleState {
  return {
    phase: "idle",
    config: {
      preference: config.preference ?? DEFAULT_TRANSFORMATION_PREFERENCE,
    },
  };
}

function rejected(
  state: GuardState,
  rejection: TransitionRejection,
): TransitionResult {
  return { state, commands: [], rejection };
}

function advanced(
  state: GuardState,
  commands: readonly GuardCommand[] = [],
): TransitionResult {
  return { state, commands };
}

function blocked(state: ActiveState, reason: BlockReason): TransitionResult {
  return advanced({
    phase: "blocked",
    config: state.config,
    transaction: state.transaction,
    reason,
  });
}

function cancelled(state: ActiveState, reason: CancelReason): TransitionResult {
  return advanced({
    phase: "cancelled",
    config: state.config,
    transaction: state.transaction,
    reason,
  });
}

function outcomeUnknown(
  state: SubmittingState,
  reason: SubmissionOutcomeUnknownReason,
): TransitionResult {
  return advanced({
    phase: "outcome_unknown",
    config: state.config,
    transaction: state.transaction,
    authorization: state.authorization,
    reason,
  });
}

function authorizedState(
  state: ActiveState,
  kind: AuthorizationKind,
  snapshotId: string,
): TransitionResult {
  return advanced({
    phase: "authorized",
    config: state.config,
    transaction: state.transaction,
    authorization: { kind, snapshotId, consumed: false },
  });
}

function authorizeTransformed(
  state: ActiveState,
  summary: ScanSummary,
): TransitionResult {
  if (summary.coverage === "unsupported") {
    return blocked(state, "unsupported_coverage");
  }
  const findings = summary.findingCount > 0;
  const candidates = summary.candidateCount > 0;
  if (findings && candidates) {
    return blocked(state, "residual_findings_and_candidates");
  }
  if (findings) return blocked(state, "residual_findings");
  if (candidates) return blocked(state, "residual_candidates");
  return authorizedState(state, "transformed", summary.snapshotId);
}

function submissionIdOf(event: GuardEvent): string | undefined {
  switch (event.type) {
    case "begin":
      return undefined;
    case "approve_original":
      return isValidOriginalApprovalConfirmation(event.confirmation)
        ? event.confirmation.submissionId
        : undefined;
    case "submit_requested":
      return isValidFinalSnapshotVerification(event.verification)
        ? event.verification.submissionId
        : undefined;
    default:
      return event.submissionId;
  }
}

function staleTransaction(
  state: ActiveState,
  event: GuardEvent,
): TransitionRejection | undefined {
  const submissionId = submissionIdOf(event);
  if (submissionId === undefined) return undefined;
  return submissionId === state.transaction.submissionId
    ? undefined
    : "stale_transaction";
}

function authorizationId(
  transaction: TransactionIdentity,
  authorization: Authorization,
): string {
  return `${transaction.submissionId}:${authorization.snapshotId}:${authorization.kind}`;
}

/**
 * True when the verification asserts the same browser context as the active
 * transaction. `submissionId` is compared separately, and `snapshotId` is the
 * exact source the authorization is bound to.
 */
function identityMatches(
  transaction: TransactionIdentity,
  verification: FinalSnapshotVerification,
): boolean {
  return (
    verification.sessionId === transaction.sessionId &&
    verification.tabId === transaction.tabId &&
    verification.frameId === transaction.frameId &&
    verification.adapterId === transaction.adapter.id &&
    verification.adapterVersion === transaction.adapter.version
  );
}

/**
 * Advances the machine by one event.
 *
 * Any event for another submission, any stale snapshot, any result for the
 * wrong scan stage, and any late async result is rejected without changing
 * state. Terminal states ignore every event, including a new `begin`.
 *
 * The caller must supply a unique submission identifier, trustworthy browser
 * identity, and exact snapshot identifiers. This pure machine compares those
 * values but cannot verify the browser sender or the source text itself.
 */
export function transition(
  state: GuardState,
  event: GuardEvent,
): TransitionResult {
  if (isTerminal(state)) return rejected(state, "terminal_state");

  switch (state.phase) {
    case "idle":
      return transitionIdle(state, event);
    case "scanning":
      return transitionScanning(state, event);
    case "awaiting_review":
      return transitionAwaitingReview(state, event);
    case "transforming":
      return transitionTransforming(state, event);
    case "revalidating":
      return transitionRevalidating(state, event);
    case "authorized":
      return transitionAuthorized(state, event);
    case "submitting":
      return transitionSubmitting(state, event);
  }
}

function transitionIdle(
  state: IdleState,
  event: GuardEvent,
): TransitionResult {
  if (event.type !== "begin") return rejected(state, "invalid_transition");
  if (!isValidTransactionIdentity(event.transaction)) {
    return rejected(state, "invalid_transaction");
  }
  return advanced(
    {
      phase: "scanning",
      config: state.config,
      transaction: event.transaction,
    },
    [
      {
        type: "scan",
        submissionId: event.transaction.submissionId,
        snapshotId: event.transaction.snapshotId,
        stage: "initial",
      },
    ],
  );
}

function transitionScanning(
  state: ScanningState,
  event: GuardEvent,
): TransitionResult {
  const stale = staleTransaction(state, event);
  if (stale !== undefined) return rejected(state, stale);

  switch (event.type) {
    case "scan_succeeded": {
      if (event.stage !== "initial") {
        return rejected(state, "stale_stage");
      }
      const summary = parseScanSummary(event.summary);
      if (summary === undefined) {
        return blocked(state, "malformed_scan_response");
      }
      if (summary.snapshotId !== state.transaction.snapshotId) {
        return rejected(state, "stale_snapshot");
      }

      const decision = evaluatePolicy(summary, state.config.preference);
      if (decision.kind === "clean") {
        return authorizedState(state, "clean", decision.snapshotId);
      }
      return advanced({
        phase: "awaiting_review",
        config: state.config,
        transaction: state.transaction,
        summary,
        decision,
      });
    }
    case "scan_failed":
      if (event.stage !== "initial") {
        return rejected(state, "stale_stage");
      }
      return event.reason === "malformed_response"
        ? blocked(state, "malformed_scan_response")
        : blocked(state, "scan_failed");
    case "timeout":
      return blocked(state, "timeout");
    case "scanner_disconnected":
      return blocked(state, "scanner_disconnected");
    case "source_changed":
      return cancelled(state, "source_changed");
    case "navigated":
      return cancelled(state, "navigated");
    case "document_replaced":
      return cancelled(state, "document_replaced");
    case "adapter_invalidated":
      return cancelled(state, "adapter_invalidated");
    case "cancel":
      return cancelled(state, "user_cancelled");
    default:
      return rejected(state, "invalid_transition");
  }
}

function transitionAwaitingReview(
  state: AwaitingReviewState,
  event: GuardEvent,
): TransitionResult {
  const stale = staleTransaction(state, event);
  if (stale !== undefined) return rejected(state, stale);

  switch (event.type) {
    case "apply_transformation":
      if (!state.decision.canTransform) {
        return rejected(state, "transformation_unavailable");
      }
      return advanced(
        {
          phase: "transforming",
          config: state.config,
          transaction: state.transaction,
          summary: state.summary,
          preference: state.decision.primaryTransformation,
        },
        [
          {
            type: "transform",
            submissionId: state.transaction.submissionId,
            snapshotId: state.summary.snapshotId,
            preference: state.decision.primaryTransformation,
          },
        ],
      );
    case "approve_original": {
      if (!state.decision.canApproveOriginal) {
        return rejected(state, "original_send_unavailable");
      }
      const confirmation = event.confirmation;
      if (!isValidOriginalApprovalConfirmation(confirmation)) {
        return rejected(state, "confirmation_required");
      }
      if (
        confirmation.submissionId !== state.transaction.submissionId ||
        confirmation.snapshotId !== state.summary.snapshotId
      ) {
        return rejected(state, "consent_mismatch");
      }
      return authorizedState(state, "original", state.summary.snapshotId);
    }
    case "review_edit":
      return cancelled(state, "user_review_edit");
    case "cancel":
      return cancelled(state, "user_cancelled");
    case "source_changed":
      return cancelled(state, "source_changed");
    case "navigated":
      return cancelled(state, "navigated");
    case "document_replaced":
      return cancelled(state, "document_replaced");
    case "adapter_invalidated":
      return cancelled(state, "adapter_invalidated");
    case "timeout":
      return blocked(state, "timeout");
    case "scanner_disconnected":
      return blocked(state, "scanner_disconnected");
    default:
      return rejected(state, "invalid_transition");
  }
}

function transitionTransforming(
  state: TransformingState,
  event: GuardEvent,
): TransitionResult {
  const stale = staleTransaction(state, event);
  if (stale !== undefined) return rejected(state, stale);

  switch (event.type) {
    case "transform_succeeded": {
      if (event.snapshotId !== state.summary.snapshotId) {
        return rejected(state, "stale_snapshot");
      }
      const transformed = event.transformedSnapshotId;
      if (
        !isValidGuardIdentifier(transformed) ||
        transformed === state.transaction.snapshotId
      ) {
        return rejected(state, "stale_snapshot");
      }
      return advanced(
        {
          phase: "revalidating",
          config: state.config,
          transaction: state.transaction,
          transformedSnapshotId: transformed,
        },
        [
          {
            type: "scan",
            submissionId: state.transaction.submissionId,
            snapshotId: transformed,
            stage: "transformed",
          },
        ],
      );
    }
    case "transform_failed":
      if (event.snapshotId !== state.summary.snapshotId) {
        return rejected(state, "stale_snapshot");
      }
      return event.reason === "malformed_response"
        ? blocked(state, "malformed_transform_response")
        : blocked(state, "transformation_failed");
    case "timeout":
      return blocked(state, "timeout");
    case "scanner_disconnected":
      return blocked(state, "scanner_disconnected");
    case "source_changed":
      return cancelled(state, "source_changed");
    case "navigated":
      return cancelled(state, "navigated");
    case "document_replaced":
      return cancelled(state, "document_replaced");
    case "adapter_invalidated":
      return cancelled(state, "adapter_invalidated");
    case "cancel":
      return cancelled(state, "user_cancelled");
    default:
      return rejected(state, "invalid_transition");
  }
}

function transitionRevalidating(
  state: RevalidatingState,
  event: GuardEvent,
): TransitionResult {
  const stale = staleTransaction(state, event);
  if (stale !== undefined) return rejected(state, stale);

  switch (event.type) {
    case "scan_succeeded": {
      if (event.stage !== "transformed") {
        return rejected(state, "stale_stage");
      }
      const summary = parseScanSummary(event.summary);
      if (summary === undefined) {
        return blocked(state, "malformed_scan_response");
      }
      if (summary.snapshotId !== state.transformedSnapshotId) {
        return rejected(state, "stale_snapshot");
      }
      return authorizeTransformed(state, summary);
    }
    case "scan_failed":
      if (event.stage !== "transformed") {
        return rejected(state, "stale_stage");
      }
      return event.reason === "malformed_response"
        ? blocked(state, "malformed_scan_response")
        : blocked(state, "scan_failed");
    case "timeout":
      return blocked(state, "timeout");
    case "scanner_disconnected":
      return blocked(state, "scanner_disconnected");
    case "source_changed":
      return cancelled(state, "source_changed");
    case "navigated":
      return cancelled(state, "navigated");
    case "document_replaced":
      return cancelled(state, "document_replaced");
    case "adapter_invalidated":
      return cancelled(state, "adapter_invalidated");
    case "cancel":
      return cancelled(state, "user_cancelled");
    default:
      return rejected(state, "invalid_transition");
  }
}

function transitionAuthorized(
  state: AuthorizedState,
  event: GuardEvent,
): TransitionResult {
  const stale = staleTransaction(state, event);
  if (stale !== undefined) return rejected(state, stale);

  switch (event.type) {
    case "submit_requested": {
      if (state.authorization.consumed) {
        return rejected(state, "authorization_consumed");
      }
      const verification = event.verification;
      if (!isValidFinalSnapshotVerification(verification)) {
        // Malformed verification is never authorization and never emits submit.
        return rejected(state, "snapshot_verification_failed");
      }
      // `staleTransaction` already rejected a different submission id above.
      if (!identityMatches(state.transaction, verification)) {
        // A verification for another browser context is unrelated or stale.
        return rejected(state, "verification_identity_mismatch");
      }
      if (verification.snapshotId !== state.authorization.snapshotId) {
        // A confirmed snapshot mismatch for the active transaction irreversibly
        // invalidates the authorization. Restoring the old source cannot revive
        // it; a new transaction and fresh scan are required.
        return blocked(state, "snapshot_mismatch");
      }
      const consumed: Authorization = {
        ...state.authorization,
        consumed: true,
      };
      return advanced(
        {
          phase: "submitting",
          config: state.config,
          transaction: state.transaction,
          authorization: consumed,
        },
        [
          {
            type: "submit",
            submissionId: state.transaction.submissionId,
            snapshotId: state.authorization.snapshotId,
            authorizationId: authorizationId(
              state.transaction,
              state.authorization,
            ),
            authorizationKind: state.authorization.kind,
          },
        ],
      );
    }
    case "timeout":
      return blocked(state, "timeout");
    case "scanner_disconnected":
      return blocked(state, "scanner_disconnected");
    case "source_changed":
      return cancelled(state, "source_changed");
    case "navigated":
      return cancelled(state, "navigated");
    case "document_replaced":
      return cancelled(state, "document_replaced");
    case "adapter_invalidated":
      return cancelled(state, "adapter_invalidated");
    case "cancel":
      return cancelled(state, "user_cancelled");
    default:
      return rejected(state, "invalid_transition");
  }
}

function transitionSubmitting(
  state: SubmittingState,
  event: GuardEvent,
): TransitionResult {
  const stale = staleTransaction(state, event);
  if (stale !== undefined) return rejected(state, stale);

  switch (event.type) {
    case "submit_succeeded":
      return advanced({
        phase: "completed",
        config: state.config,
        transaction: state.transaction,
        confirmation: "adapter_submitted",
      });
    case "submit_failed":
      // `rejected` is an adapter-level assertion that the command was refused
      // before dispatch. `transport_error` may have happened after dispatch, so
      // the external outcome is unknown.
      return event.reason === "transport_error"
        ? outcomeUnknown(state, "transport_error")
        : blocked(state, "submission_failed");
    case "submit_requested":
      return rejected(state, "authorization_consumed");
    case "cancel":
      return outcomeUnknown(state, "cancelled");
    case "timeout":
      return outcomeUnknown(state, "timeout");
    case "scanner_disconnected":
      return outcomeUnknown(state, "scanner_disconnected");
    case "source_changed":
      return outcomeUnknown(state, "source_changed");
    case "navigated":
      return outcomeUnknown(state, "navigated");
    case "document_replaced":
      return outcomeUnknown(state, "document_replaced");
    case "adapter_invalidated":
      return outcomeUnknown(state, "adapter_invalidated");
    default:
      return rejected(state, "invalid_transition");
  }
}
